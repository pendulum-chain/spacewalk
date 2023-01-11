use orml_oracle::{TimestampedValue, DataProviderExtended};
pub use primitives::{oracle::Key as OracleKey, CurrencyId, TruncateFixedPointToInt};
use sp_std::{
	marker
};
use dia_oracle::DiaOracle;

use sp_runtime::{
	traits::{Convert},
};

struct X;

impl Convert<OracleKey, Option<(Vec<u8>,Vec<u8>)>> for X{
    fn convert(a: OracleKey) -> Option<(Vec<u8>,Vec<u8>)> {
        todo!()
    }
}

impl Convert<(Vec<u8>,Vec<u8>), Option<OracleKey>> for X{
    fn convert(a: (Vec<u8>,Vec<u8>)) -> Option<OracleKey> {
        todo!()
    }
}

pub struct DiaOracleAdapter<DiaPallet : DiaOracle, UnsignedFixedPoint, Moment, ConvertKey>(marker::PhantomData<(DiaPallet, UnsignedFixedPoint, Moment, ConvertKey)>);

impl<T: DiaOracle, UnsignedFixedPoint, Moment, ConvertKey> DataProviderExtended<OracleKey, TimestampedValue<UnsignedFixedPoint, Moment>> for DiaOracleAdapter<T, UnsignedFixedPoint, Moment, ConvertKey>
    where ConvertKey : Convert<OracleKey, Option<(Vec<u8>,Vec<u8>)>> + Convert<(Vec<u8>,Vec<u8>), Option<OracleKey>>
{
    fn get_no_op(key: &OracleKey) -> Option<TimestampedValue<UnsignedFixedPoint, Moment>> {
        let dia_key : Option<(Vec<u8>,Vec<u8>)> = ConvertKey::convert(key.clone());
        todo!()
    }

    fn get_all_values() -> Vec<(OracleKey, Option<TimestampedValue<UnsignedFixedPoint, Moment>>)> {
        // let r = T::get_coin_info(blockchain, symbol);
        todo!()
    }
}



