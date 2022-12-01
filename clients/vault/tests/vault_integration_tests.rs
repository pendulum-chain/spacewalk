use std::time::Duration;

use async_trait::async_trait;
use frame_support::assert_ok;
use futures::Future;
use sp_keyring::AccountKeyring;

use runtime::{
	integration::*, types::*, CurrencyId::Token, FixedPointNumber, FixedU128, SpacewalkParachain,
	UtilFuncs, VaultRegistryPallet,
};
use stellar_relay_lib::sdk::SecretKey;
use wallet::StellarWallet;

const TIMEOUT: Duration = Duration::from_secs(90);

const DEFAULT_NATIVE_CURRENCY: CurrencyId = Token(TokenSymbol::AMPE);
const DEFAULT_TESTING_CURRENCY: CurrencyId = Token(TokenSymbol::KSM);
const DEFAULT_WRAPPED_CURRENCY: CurrencyId = CurrencyId::AlphaNum4 {
	code: *b"USDC",
	issuer: [
		20, 209, 150, 49, 176, 55, 23, 217, 171, 154, 54, 110, 16, 50, 30, 226, 102, 231, 46, 199,
		108, 171, 97, 144, 240, 161, 51, 109, 72, 34, 159, 139,
	],
};

const STELLAR_VAULT_SECRET_KEY: &str = "SB6WHKIU2HGVBRNKNOEOQUY4GFC4ZLG5XPGWLEAHTIZXBXXYACC76VSQ";

async fn test_with<F, R>(execute: impl FnOnce(SubxtClient) -> F) -> R
where
	F: Future<Output = R>,
{
	service::init_subscriber();
	let (client, _tmp_dir) = default_provider_client(AccountKeyring::Alice).await;

	// Has to be Bob because he is set as `authorized_oracle` in the genesis config
	let parachain_rpc = setup_provider(client.clone(), AccountKeyring::Bob).await;

	set_exchange_rate_and_wait(
		&parachain_rpc,
		DEFAULT_TESTING_CURRENCY,
		FixedU128::from(100000000),
	)
	.await;
	set_exchange_rate_and_wait(
		&parachain_rpc,
		DEFAULT_NATIVE_CURRENCY,
		FixedU128::saturating_from_rational(1u128, 100u128),
	)
	.await;
	set_stellar_fees(&parachain_rpc, FixedU128::from(1)).await;

	execute(client).await
}

async fn test_with_vault<F, R>(
	execute: impl FnOnce(SubxtClient, VaultId, SpacewalkParachain) -> F,
) -> R
where
	F: Future<Output = R>,
{
	service::init_subscriber();
	let (client, _tmp_dir) = default_provider_client(AccountKeyring::Alice).await;

	let parachain_rpc = setup_provider(client.clone(), AccountKeyring::Bob).await;
	set_exchange_rate_and_wait(
		&parachain_rpc,
		DEFAULT_TESTING_CURRENCY,
		FixedU128::from(100000000),
	)
	.await;
	set_exchange_rate_and_wait(
		&parachain_rpc,
		DEFAULT_NATIVE_CURRENCY,
		FixedU128::saturating_from_rational(1u128, 100u128),
	)
	.await;
	set_stellar_fees(&parachain_rpc, FixedU128::from(1)).await;

	let vault_provider = setup_provider(client.clone(), AccountKeyring::Charlie).await;

	let account_id = AccountKeyring::Charlie.to_raw_public();
	// Convert to the subxt AccountId type because unfortunately there is a version mismatch between
	// the sp_xxx dependencies subxt uses and the ones we use
	let account_id = subxt::ext::sp_runtime::AccountId32::from(account_id);

	let vault_id = VaultId::new(account_id, DEFAULT_TESTING_CURRENCY, DEFAULT_WRAPPED_CURRENCY);

	execute(client, vault_id, vault_provider).await
}

#[tokio::test(flavor = "multi_thread")]
async fn test_issue_overpayment_succeeds() {
	test_with_vault(|client, vault_id, vault_provider| async move {
		let relayer_provider = setup_provider(client.clone(), AccountKeyring::Bob).await;
		let user_provider = setup_provider(client.clone(), AccountKeyring::Dave).await;

		let secret = SecretKey::from_encoding(STELLAR_VAULT_SECRET_KEY).unwrap();
		let public_key = secret.get_public().clone().into_binary();

		let issue_amount = 100000;
		let over_payment_factor = 3;
		let vault_collateral = get_required_vault_collateral_for_issue(
			&vault_provider,
			issue_amount * over_payment_factor,
			vault_id.collateral_currency(),
		)
		.await;

		assert_ok!(
			vault_provider
				.register_vault_with_public_key(&vault_id, vault_collateral, public_key)
				.await
		);

		let issue = user_provider.request_issue(issue_amount, &vault_id).await.unwrap();

		wallet
			.send_payment_to_address(
				issue.vault_address.to_address(btc_rpc.network()).unwrap(),
				(issue.amount + issue.fee) as u64 * over_payment_factor as u64,
				issue.id,
			)
			.await
			.unwrap();

		let metadata = btc_rpc.send_to_address().await.unwrap();

		join(
			assert_event::<EndowedEvent, _>(TIMEOUT, user_provider.clone(), |x| {
				if &x.who == user_provider.get_account_id() {
					assert_eq!(x.amount, issue.amount * over_payment_factor);
					true
				} else {
					false
				}
			}),
			user_provider
				.execute_issue(issue.issue_id, &metadata.proof, &metadata.raw_tx)
				.map(Result::unwrap),
		)
		.await;
	})
	.await;
}

type StellarPublicKey = [u8; 32];

#[async_trait]
trait SpacewalkParachainExt {
	async fn register_vault_with_public_key(
		&self,
		vault_id: &VaultId,
		collateral: u128,
		public_key: StellarPublicKey,
	) -> Result<(), runtime::Error>;
}

#[async_trait]
impl SpacewalkParachainExt for SpacewalkParachain {
	async fn register_vault_with_public_key(
		&self,
		vault_id: &VaultId,
		collateral: u128,
		public_key: StellarPublicKey,
	) -> Result<(), runtime::Error> {
		self.register_public_key(public_key).await.unwrap();
		self.register_vault(vault_id, collateral).await.unwrap();
		Ok(())
	}
}
