use crate::horizon::{HorizonClient};
use crate::error::Error;
use async_trait::async_trait;
use substrate_stellar_sdk::{Asset, Claimant, ClaimPredicate, IntoAmount, Memo, Operation, PublicKey, StellarSdkError, StroopAmount, Transaction};
use substrate_stellar_sdk::compound_types::LimitedString;
use substrate_stellar_sdk::types::{Preconditions, SequenceNumber};
use primitives::{Amount, derive_shortened_request_id, StellarStroops};

#[async_trait]
pub trait HorizonClientExt : HorizonClient {

    async fn is_claimable_balance_op_required(&self,
                          destination_address:PublicKey,
                          is_public_network:bool,
                          to_be_redeemed_asset:Asset,
                          to_be_redeemed_amount: StellarStroops,
    ) -> Option<Vec<Operation>> {
        // convert pubkey to string
        let dest_addr_encoded = destination_address.to_encoding();
        let dest_addr_str =
            std::str::from_utf8(&dest_addr_encoded).unwrap();

        // check if account exists
        let mut ops = match self.get_account(dest_addr_str, is_public_network).await {
            Ok(account)  if account.is_trustline_exist(&to_be_redeemed_asset) => {
                // normal operation
                return None;
            }
            Err(Error::HorizonSubmissionError { title:_, status, reason: _, envelope_xdr:_ })
            if status == 404  => {
                // create an account for the destination
                let create_account_op = create_account_operation(
                    destination_address.clone(),0
                        ).unwrap();
                let ops = vec![create_account_op];

                // check if we need to create a claimable balance
                let to_be_redeemed_amount = primitives::Balance::try_from(to_be_redeemed_amount).unwrap();
                if &to_be_redeemed_asset == &Asset::AssetTypeNative &&
                    to_be_redeemed_amount >= primitives::CurrencyId::StellarNative.one() {
                    // normal payment operation, then return
                    return Some(ops);
                } else {
                    ops
                }
            }
            _ => { vec![] }
        };

        let claimant = Claimant::new(
            destination_address,ClaimPredicate::ClaimPredicateUnconditional
        ).unwrap();
        // create claimable balance operation
        // let to_be_redeemed_amount = i64::try_from(to_be_redeemed_amount).unwrap();
        let create_claim_bal_op = create_claimable_balance_operation(
            to_be_redeemed_asset,to_be_redeemed_amount,vec![claimant]
        ).unwrap();

        ops.push(create_claim_bal_op);

        Some(ops)
    }
}

fn change_error_type<T>(item:Result<T,StellarSdkError>) -> Result<T, Error> {
    item.map_err(|e| Error::BuildTransactionError(
        format!("Failed to create operation: {e:?}")
    ))
}

pub fn create_claimable_balance_operation(asset: Asset, stroop_amount:StellarStroops, claimants:Vec<Claimant>) -> Result<Operation,Error> {
    let amount = StroopAmount(stroop_amount);
    change_error_type(
        Operation::new_create_claimable_balance(
            asset,amount,claimants
        ))
}

pub fn create_account_operation(destination_address: PublicKey, starting_stroop_amount: StellarStroops) -> Result<Operation,Error> {
    let amount = StroopAmount(starting_stroop_amount);
    change_error_type(
        Operation::new_create_account(
        destination_address,amount
    ))
}

pub fn create_payment_operation(
    destination_address: PublicKey,
    asset: Asset,
    stroop_amount: StellarStroops,
    public_key: PublicKey,
) -> Result<Operation,Error> {
    let stroop_amount = StroopAmount(stroop_amount);

    change_error_type(
    Operation::new_payment(destination_address, asset, stroop_amount)
    )?
    .set_source_account(public_key)
    .map_err(|e| {
        Error::BuildTransactionError(format!("Setting source account failed: {e:?}"))
    })
}

pub fn create_basic_transaction(
    request_id: [u8; 32],
    stroop_fee_per_operation: u32,
    public_key: PublicKey,
    next_sequence_number: SequenceNumber,
) -> Result<Transaction, Error> {
    let memo_text = Memo::MemoText(
        LimitedString::new(derive_shortened_request_id(&request_id))
            .map_err(|_| Error::BuildTransactionError("Invalid hash".to_string()))?,
    );

    let transaction = change_error_type(Transaction::new(
        public_key.clone(),
        next_sequence_number,
        Some(stroop_fee_per_operation),
        Preconditions::PrecondNone,
        Some(memo_text),
    ))?;

    Ok(transaction)
}


pub trait AppendExt<T> {
    fn append_multiple(&mut self, items:Vec<T>) -> Result<(),Error>;
    fn append(&mut self, item:T) -> Result<(),Error>;
}

impl AppendExt<Operation> for Transaction {
    fn append_multiple(&mut self, items: Vec<Operation>) -> Result<(), Error> {
        for operation in items {
            self.append(operation)?;
        }

        Ok(())
    }

    fn append(&mut self, item: Operation) -> Result<(), Error> {
        self.append_operation(item)
            .map_err(|e| {
                Error::BuildTransactionError(
                    format!("Appending payment operation failed: {e:?}"))
            })?;

        Ok(())
    }
}