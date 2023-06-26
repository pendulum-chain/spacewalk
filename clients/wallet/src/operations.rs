use crate::horizon::{HorizonAccountResponse, HorizonClient};
use crate::error::Error;
use async_trait::async_trait;
use substrate_stellar_sdk::{Asset, Claimant, Memo, Operation, PublicKey, StellarSdkError, StroopAmount, Transaction};
use substrate_stellar_sdk::compound_types::LimitedString;
use substrate_stellar_sdk::types::{Preconditions, SequenceNumber};
use primitives::{Amount, CurrencyId, CurrencyInfo, derive_shortened_request_id};
use crate::StellarWallet;

#[async_trait]
pub trait HorizonClientExt : HorizonClient {

    async fn is_account_active(&self) -> bool;

    async fn start(&self,
                   dest_account_encoded:&str,
                   to_be_redeemed_currency_id:CurrencyId,
                   to_be_redeemed_amount: Amount
    ) -> Result<(),Error> {
     match self.get_account(dest_account_encoded).await {
         Err(Error::HorizonSubmissionError { title:_, status, reason:_, envelope_xdr:_ })
         if status == 404 => {
             // account is inactive
             if to_be_redeemed_currency_id == CurrencyId::StellarNative &&
                 to_be_redeemed_amount >= 1 {
                 // create an account
             }

         }
         Err(e) => {}
         Ok(account) => {
             let asset = to_be_redeemed_currency_id.try_into().

             if to_be_redeemed_currency_id != CurrencyId::StellarNative {

                 for b in  account.balances.iter() {
                     b.asset_code == to_be_redeemed_currency_id.symbol() {

                     }
                 }

             }

         }
     }
        // account is inactive
        tracing::warn!("Problem fetching account {dest_account_encoded}: {e:?}");

        // create this account

        Ok(())
    }
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

fn change_error_type<T>(item:Result<T,StellarSdkError>) -> Result<T, Error> {
    item.map_err(|e| Error::BuildTransactionError(
        format!("Failed to create operation: {e:?}")
    ))
}

pub fn create_claimable_balance_operation(asset: Asset, stroop_amount:i64, claimants:Vec<Claimant>) -> Result<Operation,Error> {
    let amount = StroopAmount(stroop_amount);
    change_error_type(
        Operation::new_create_claimable_balance(
            asset,amount,claimants
        ))
}

pub fn create_account_operation(destination_address: PublicKey, starting_stroop_amount: i64) -> Result<Operation,Error> {
    let amount = StroopAmount(starting_stroop_amount);
    change_error_type(
        Operation::new_create_account(
        destination_address,amount
    ))
}

pub fn create_payment_operation(
    destination_address: PublicKey,
    asset: Asset,
    stroop_amount: i64,
    public_key: PublicKey,
) -> Result<Operation,Error> {
    let amount = StroopAmount(stroop_amount);

    change_error_type(
    Operation::new_payment(destination_address, asset, amount)
    )?
    .set_source_account(public_key)
    .map_err(|e| {
        Error::BuildTransactionError(format!("Setting source account failed: {e:?}"))
    })
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