use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::traits::Get;
use scale_info::TypeInfo;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use currency::Amount;
pub use primitives::issue::{IssueRequest, IssueRequestStatus};
use primitives::VaultId;
use vault_registry::types::CurrencyId;

use crate::Config;

pub(crate) type BalanceOf<T> = <T as vault_registry::Config>::Balance;

pub(crate) type DefaultVaultId<T> = VaultId<<T as frame_system::Config>::AccountId, CurrencyId<T>>;

#[derive(Encode, Decode, Clone, PartialEq, TypeInfo, MaxEncodedLen)]
#[cfg_attr(feature = "std", derive(Debug, Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub enum IssueRequestStatus {
	/// opened, but not yet executed or cancelled
	Pending,
	/// payment was received
	Completed,
	/// payment was not received, vault may receive griefing collateral
	Cancelled,
}
impl Default for IssueRequestStatus {
	fn default() -> Self {
		IssueRequestStatus::Pending
	}
}

#[derive(Encode, Decode, Clone, PartialEq, TypeInfo, MaxEncodedLen)]
#[cfg_attr(feature = "std", derive(Debug, Serialize, Deserialize))]
pub struct IssueRequest<AccountId, BlockNumber, Balance, CurrencyId: Copy> {
	/// the vault associated with this issue request
	pub vault: VaultId<AccountId, CurrencyId>,
	/// the *active* block height when this request was opened
	pub opentime: BlockNumber,
	/// the issue period when this request was opened
	pub period: BlockNumber,
	#[cfg_attr(feature = "std", serde(bound(deserialize = "Balance: std::str::FromStr")))]
	#[cfg_attr(feature = "std", serde(deserialize_with = "deserialize_from_string"))]
	#[cfg_attr(feature = "std", serde(bound(serialize = "Balance: std::fmt::Display")))]
	#[cfg_attr(feature = "std", serde(serialize_with = "serialize_as_string"))]
	/// the collateral held for spam prevention
	pub griefing_collateral: Balance,
	#[cfg_attr(feature = "std", serde(bound(deserialize = "Balance: std::str::FromStr")))]
	#[cfg_attr(feature = "std", serde(deserialize_with = "deserialize_from_string"))]
	#[cfg_attr(feature = "std", serde(bound(serialize = "Balance: std::fmt::Display")))]
	#[cfg_attr(feature = "std", serde(serialize_with = "serialize_as_string"))]
	/// the number of tokens that will be transferred to the user (as such, this does not include
	/// the fee)
	pub amount: Balance,
	#[cfg_attr(feature = "std", serde(bound(deserialize = "Balance: std::str::FromStr")))]
	#[cfg_attr(feature = "std", serde(deserialize_with = "deserialize_from_string"))]
	#[cfg_attr(feature = "std", serde(bound(serialize = "Balance: std::fmt::Display")))]
	#[cfg_attr(feature = "std", serde(serialize_with = "serialize_as_string"))]
	/// the number of tokens that will be transferred to the fee pool
	pub fee: Balance,
	/// the account issuing tokens
	pub requester: AccountId,
	/// the vault's Bitcoin deposit address
	pub btc_address: BtcAddress,
	/// the vault's Bitcoin public key (when this request was made)
	pub btc_public_key: BtcPublicKey,
	/// the highest recorded height in the BTC-Relay (at time of opening)
	pub btc_height: u32,
	/// the status of this issue request
	pub status: IssueRequestStatus,
}

#[cfg(feature = "std")]
fn serialize_as_string<S: Serializer, T: std::fmt::Display>(
	t: &T,
	serializer: S,
) -> Result<S::Ok, S::Error> {
	serializer.serialize_str(&t.to_string())
}

#[cfg(feature = "std")]
fn deserialize_from_string<'de, D: Deserializer<'de>, T: std::str::FromStr>(
	deserializer: D,
) -> Result<T, D::Error> {
	let s = String::deserialize(deserializer)?;
	s.parse::<T>().map_err(|_| serde::de::Error::custom("Parse from string failed"))
}

pub type DefaultIssueRequest<T> = IssueRequest<
	<T as frame_system::Config>::AccountId,
	<T as frame_system::Config>::BlockNumber,
	BalanceOf<T>,
	CurrencyId<T>,
>;

pub trait IssueRequestExt<T: Config> {
	fn amount(&self) -> Amount<T>;
	fn fee(&self) -> Amount<T>;
	fn griefing_collateral(&self) -> Amount<T>;
}

impl<T: Config> IssueRequestExt<T> for DefaultIssueRequest<T> {
	fn amount(&self) -> Amount<T> {
		Amount::new(self.amount, self.vault.wrapped_currency())
	}
	fn fee(&self) -> Amount<T> {
		Amount::new(self.fee, self.vault.wrapped_currency())
	}
	fn griefing_collateral(&self) -> Amount<T> {
		Amount::new(self.griefing_collateral, T::GetGriefingCollateralCurrencyId::get())
	}
}
