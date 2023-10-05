use stellar_relay_lib::sdk::types::{GeneralizedTransactionSet, TransactionSet};
use stellar_relay_lib::sdk::XdrCodec;

use crate::oracle::Error;

pub type Base64EncodedTxSet = String;

pub trait TxSetBase64Codec : XdrCodec + Sized {
    const XDR_PREFIX:u8;
    const DECODE_ERROR_TITLE:&'static str;
    /// returns a base64 encoded xdr string
    ///
    /// first gets the xdr version of the object
    /// adds a prefix u8 value on that xdr `Vec<u8>`
    ///     * 0 for `TxSet`,
    ///     * 1 for `GeneralizedTxSet`
    /// encode it to base64
    fn to_base64_encoded_xdr_string_for_mapping(&self) -> Base64EncodedTxSet {
        let mut final_txset_xdr =  vec![];
        // we prepend a prefix value to this encoded data to indicate its type.
        final_txset_xdr.push(Self::XDR_PREFIX);

        let mut txset_xdr = self.to_xdr();
        final_txset_xdr.append(&mut txset_xdr);

        base64::encode(final_txset_xdr)

    }

    /// decodes a base64 string
    /// removes the prefix value of the xdr
    /// and then converts to either `TxSet` or `GeneralizedTxSet`
    fn from_base64_encoded_xdr_string_for_mapping(base64_encoded_tx_set:&Base64EncodedTxSet) -> Result<Self,Error> {
        let mut final_txset_xdr = base64::decode(base64_encoded_tx_set)
            .map_err(|e| Error::DecodeError(format!("{}{e:?}",Self::DECODE_ERROR_TITLE)))?;

        if final_txset_xdr.remove(0) != Self::XDR_PREFIX {
            return Err(Error::DecodeError(format!(
                "{}Prefix is not {} of this encoded set: {base64_encoded_tx_set}",
                Self::DECODE_ERROR_TITLE,
                Self::XDR_PREFIX
            )));
        }

        Self::from_xdr(final_txset_xdr)
            .map_err(|e| Error::DecodeError(format!("{}{e:?}", Self::DECODE_ERROR_TITLE)))
    }

}

impl TxSetBase64Codec for GeneralizedTransactionSet {
    const XDR_PREFIX: u8 = 1;
    const DECODE_ERROR_TITLE: &'static str = "Cannot decode to GeneralizedTxSet: ";
}

impl TxSetBase64Codec for TransactionSet {
    const XDR_PREFIX: u8 = 0;
    const DECODE_ERROR_TITLE: &'static str = "Cannot decode to TxSet: ";
}

#[cfg(test)]
mod tests {
    use stellar_relay_lib::sdk::types::{GeneralizedTransactionSet, TransactionSet};
    use stellar_relay_lib::sdk::XdrCodec;
    use super::TxSetBase64Codec;

    const GENERALIZED_TX_SET: &str = "AAAAAVUcNbdctOS+OftZngTJY07YUUqb4P1I/owUmgdMuzscAAAAAgAAAAAAAAABAAAAAAAAAAEAAAAAAAAAZAAAAAEAAAACAAAAAMgOELhl9VFf5x0pG1aY8Mm/QQcnigdQ9MgWM1F8c6HSAAAAZAAZMt8AADrRAAAAAQAAAAAAAAAAAAAAAGUb2+8AAAABAAAAG3RzOjIwMjMtMTAtMDNUMDk6MTU6MDEuNTkyWgAAAAABAAAAAAAAAAEAAAAAf4MDV2AZH1oB1nouL9LSGUHGGafzcb48GXQyWFd9zswAAAAAAAAAAACYloAAAAAAAAAAAXxzodIAAABAEq3w/8HQ6kjqooVJPjg1TquL2pMOT+P9P7a3HpdqUYyFyJ8F32igbhIu3jvIJkafhDTosuL/rid2BxmScxhfDwAAAAAAAAAA";

    fn example() -> GeneralizedTransactionSet {
        let to_vec_u8 = GENERALIZED_TX_SET.as_bytes().to_vec();
        GeneralizedTransactionSet::from_base64_xdr(to_vec_u8).expect("should return a GeneralizedTransactionSet")
    }

    #[test]
    fn encode_decode_generalized_transaction_set() {
        let example = example();

        let res = example.to_base64_encoded_xdr_string_for_mapping();
        let x = GeneralizedTransactionSet::from_base64_encoded_xdr_string_for_mapping(&res)
            .expect("should be able to decode correctly");
        let res = x.to_base64_encoded_xdr_string()
            .expect("should be able to convert to base64.");
        assert_eq!(&res, GENERALIZED_TX_SET);
        println!("value of gen tx set: {res}");
    }

    #[test]
    fn encode_decode_fails_for_transaction_set() {
        let example = example();

        let res = example.to_base64_encoded_xdr_string_for_mapping();
        assert!(
            TransactionSet::from_base64_encoded_xdr_string_for_mapping(&res)
                .is_err()
        );
    }

}

