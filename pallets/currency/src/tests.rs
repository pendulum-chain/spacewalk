use sp_runtime::{traits::StaticLookup, FixedPointNumber};

use primitives::{
	stellar::{
		compound_types::LimitedVarArray,
		types::{
			ClaimantV0, CreateClaimableBalanceOp, OperationBody, PaymentOp, Preconditions,
			TransactionExt, TransactionV1Envelope,
		},
		ClaimPredicate, Claimant, Memo, MuxedAccount, Operation, PublicKey, Transaction,
		TransactionEnvelope,
	},
	AssetIssuer, Bytes4,
};

use crate::{mock::*, Amount, Config};

#[test]
fn test_get_amount_from_transaction_envelope_works() {
	run_test(|| {
		let code: Bytes4 = *b"USDC";
		let issuer: AssetIssuer = [0; 32];
		let currency = CurrencyId::AlphaNum4 { code, issuer };
		let asset = <Test as Config>::AssetConversion::lookup(currency).unwrap();
		let recipient_stellar_address = [1u8; 32];
		let source_account = MuxedAccount::KeyTypeEd25519(recipient_stellar_address);

		// the amount contained in each operation
		let stroop_amount: i64 = 100_000;
		// the amount that the stroops represent on chain
		let native_amount = <Test as Config>::BalanceConversion::unlookup(stroop_amount);
		let operations = LimitedVarArray::new(vec![
			Operation {
				source_account: Some(source_account.clone()),
				body: OperationBody::Payment(PaymentOp {
					destination: MuxedAccount::KeyTypeEd25519(recipient_stellar_address),
					asset: asset.clone(),
					amount: stroop_amount,
				}),
			},
			Operation {
				source_account: Some(source_account.clone()),
				body: OperationBody::CreateClaimableBalance(CreateClaimableBalanceOp {
					asset: asset.clone(),
					amount: stroop_amount,
					claimants: LimitedVarArray::new(vec![Claimant::ClaimantTypeV0(ClaimantV0 {
						destination: PublicKey::PublicKeyTypeEd25519(recipient_stellar_address),
						predicate: ClaimPredicate::ClaimPredicateUnconditional,
					})])
					.unwrap(),
				}),
			},
		])
		.unwrap();

		let tx_env = TransactionEnvelope::EnvelopeTypeTx(TransactionV1Envelope {
			tx: Transaction {
				source_account,
				fee: 0,
				seq_num: 0,
				cond: Preconditions::PrecondNone,
				memo: Memo::MemoNone,
				operations,
				ext: TransactionExt::V0,
			},
			signatures: LimitedVarArray::new_empty(),
		});

		let result = Currency::get_amount_from_transaction_envelope(
			&tx_env,
			recipient_stellar_address,
			currency,
		);
		let result_amount: Amount<Test> =
			result.expect("Failed to get amount from transaction envelope");

		// We expect the result to be the sum of the two operations
		assert_eq!(result_amount.amount(), native_amount * 2);
		assert_eq!(result_amount.currency(), currency);
	})
}

#[test]
fn test_get_amount_from_transaction_envelope_works_for_mismatching_assets() {
	run_test(|| {
		let code: Bytes4 = *b"USDC";
		let issuer: AssetIssuer = [0; 32];
		let currency = CurrencyId::AlphaNum4 { code, issuer };

		// use a different asset when creating the transaction
		let other_issuer: AssetIssuer = [1; 32];
		let other_currency = CurrencyId::AlphaNum4 { code, issuer: other_issuer };
		let asset = <Test as Config>::AssetConversion::lookup(other_currency).unwrap();
		let recipient_stellar_address = [1u8; 32];
		let source_account = MuxedAccount::KeyTypeEd25519(recipient_stellar_address);

		// the amount contained in each operation
		let stroop_amount: i64 = 100_000;
		let operations = LimitedVarArray::new(vec![
			Operation {
				source_account: Some(source_account.clone()),
				body: OperationBody::Payment(PaymentOp {
					destination: MuxedAccount::KeyTypeEd25519(recipient_stellar_address),
					asset: asset.clone(),
					amount: stroop_amount,
				}),
			},
			Operation {
				source_account: Some(source_account.clone()),
				body: OperationBody::CreateClaimableBalance(CreateClaimableBalanceOp {
					asset: asset.clone(),
					amount: stroop_amount,
					claimants: LimitedVarArray::new(vec![Claimant::ClaimantTypeV0(ClaimantV0 {
						destination: PublicKey::PublicKeyTypeEd25519(recipient_stellar_address),
						predicate: ClaimPredicate::ClaimPredicateUnconditional,
					})])
					.unwrap(),
				}),
			},
		])
		.unwrap();

		let tx_env = TransactionEnvelope::EnvelopeTypeTx(TransactionV1Envelope {
			tx: Transaction {
				source_account,
				fee: 0,
				seq_num: 0,
				cond: Preconditions::PrecondNone,
				memo: Memo::MemoNone,
				operations,
				ext: TransactionExt::V0,
			},
			signatures: LimitedVarArray::new_empty(),
		});

		let result = Currency::get_amount_from_transaction_envelope(
			&tx_env,
			recipient_stellar_address,
			currency,
		);
		let result_amount: Amount<Test> =
			result.expect("Failed to get amount from transaction envelope");

		// We expect the result to be 0 because the assets don't match, thus the transaction did not
		// contain any amount for the requested currency
		assert_eq!(result_amount.amount(), 0);
		assert_eq!(result_amount.currency(), currency);
	})
}

#[test]
fn test_checked_fixed_point_mul_rounded_up() {
	run_test(|| {
		let currency = CurrencyId::Native;
		let tests: Vec<(Amount<Test>, UnsignedFixedPoint, Amount<Test>)> = vec![
			(
				Amount::new(10, currency),
				UnsignedFixedPoint::checked_from_rational(1, 3).unwrap(),
				Amount::new(4, currency),
			),
			(
				Amount::new(9, currency),
				UnsignedFixedPoint::checked_from_rational(1, 3).unwrap(),
				Amount::new(3, currency),
			),
			(
				Amount::new(10, currency),
				UnsignedFixedPoint::checked_from_rational(1, UnsignedFixedPoint::accuracy())
					.unwrap(),
				Amount::new(1, currency),
			),
			(Amount::new(10, currency), UnsignedFixedPoint::from(0), Amount::new(0, currency)),
			(
				Amount::new(UnsignedFixedPoint::accuracy(), currency),
				UnsignedFixedPoint::checked_from_rational(1, UnsignedFixedPoint::accuracy())
					.unwrap(),
				Amount::new(1, currency),
			),
			(
				Amount::new(UnsignedFixedPoint::accuracy() + 1, currency),
				UnsignedFixedPoint::checked_from_rational(1, UnsignedFixedPoint::accuracy())
					.unwrap(),
				Amount::new(2, currency),
			),
		];

		for (amount, percent, expected) in tests {
			let actual = amount.checked_fixed_point_mul_rounded_up(&percent).unwrap();
			assert_eq!(actual, expected);
		}
	})
}

#[test]
fn test_checked_fixed_point_mul() {
	run_test(|| {
		let currency = CurrencyId::Native;
		let tests: Vec<(Amount<Test>, UnsignedFixedPoint, Amount<Test>)> = vec![
			(
				Amount::new(1 * 10u128.pow(8), currency), // 1 BTC
				UnsignedFixedPoint::checked_from_rational(1, 2).unwrap(), // 50%
				Amount::new(50000000, currency),
			),
			(
				Amount::new(50000000, currency),                            // 0.5 BTC
				UnsignedFixedPoint::checked_from_rational(5, 100).unwrap(), // 5%
				Amount::new(2500000, currency),
			),
			(
				Amount::new(25000000, currency), // 0.25 BTC
				UnsignedFixedPoint::checked_from_rational(5, 1000).unwrap(), // 0.5%
				Amount::new(125000, currency),
			),
			(
				Amount::new(12500000, currency), // 0.125 BTC
				UnsignedFixedPoint::checked_from_rational(5, 100000).unwrap(), // 0.005%
				Amount::new(625, currency),
			),
			(
				Amount::new(1 * 10u128.pow(10), currency), // 1 DOT
				UnsignedFixedPoint::checked_from_rational(1, 10).unwrap(), // 10%
				Amount::new(1000000000, currency),
			),
		];

		for (amount, percent, expected) in tests {
			let actual = amount.checked_fixed_point_mul(&percent).unwrap();
			assert_eq!(actual, expected);
		}
	})
}
