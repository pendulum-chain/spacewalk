use orml_oracle::{TimestampedValue, DataProviderExtended};
pub use primitives::{oracle::Key as OracleKey, CurrencyId, TruncateFixedPointToInt};
use sp_std::{
	marker
};

trait X{

}

struct DiaOracleAdapter<DiaPallet : X, UnsignedFixedPoint, Moment>(marker::PhantomData<(DiaPallet, UnsignedFixedPoint, Moment)>);

impl<T: X, UnsignedFixedPoint, Moment> DataProviderExtended<OracleKey, TimestampedValue<UnsignedFixedPoint, Moment>> for DiaOracleAdapter<T, UnsignedFixedPoint, Moment >{
    fn get_no_op(key: &OracleKey) -> Option<TimestampedValue<UnsignedFixedPoint, Moment>> {
        todo!()
    }

    fn get_all_values() -> Vec<(OracleKey, Option<TimestampedValue<UnsignedFixedPoint, Moment>>)> {
        todo!()
    }
}

