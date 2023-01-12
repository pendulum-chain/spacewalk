use dia_oracle::DiaOracle;
use orml_oracle::{DataProviderExtended, TimestampedValue};
pub use primitives::{oracle::Key as OracleKey, CurrencyId, TruncateFixedPointToInt};
use sp_std::marker;

use sp_runtime::traits::Convert;

pub struct DiaOracleConvertor;

impl Convert<OracleKey, Option<(Vec<u8>, Vec<u8>)>> for DiaOracleConvertor {
	fn convert(a: OracleKey) -> Option<(Vec<u8>, Vec<u8>)> {
		todo!()
	}
}

impl Convert<(Vec<u8>, Vec<u8>), Option<OracleKey>> for DiaOracleConvertor {
	fn convert(a: (Vec<u8>, Vec<u8>)) -> Option<OracleKey> {
		todo!()
	}
}

pub struct DiaOracleAdapter<
	DiaPallet: DiaOracle,
	UnsignedFixedPoint,
	Moment,
	ConvertKey,
	ConvertPrice,
	ConvertMoment,
>(
	marker::PhantomData<(
		DiaPallet,
		UnsignedFixedPoint,
		Moment,
		ConvertKey,
		ConvertPrice,
		ConvertMoment,
	)>,
);

impl<Dia, UnsignedFixedPoint, Moment, ConvertKey, ConvertPrice, ConvertMoment>
	DataProviderExtended<OracleKey, TimestampedValue<UnsignedFixedPoint, Moment>>
	for DiaOracleAdapter<Dia, UnsignedFixedPoint, Moment, ConvertKey, ConvertPrice, ConvertMoment>
where
	Dia: DiaOracle,
	ConvertKey: Convert<OracleKey, Option<(Vec<u8>, Vec<u8>)>>
		+ Convert<(Vec<u8>, Vec<u8>), Option<OracleKey>>,
	ConvertPrice:
		Convert<UnsignedFixedPoint, Option<u128>> + Convert<u128, Option<UnsignedFixedPoint>>,
	ConvertMoment: Convert<Moment, Option<u64>> + Convert<u64, Option<Moment>>,
{
	fn get_no_op(key: &OracleKey) -> Option<TimestampedValue<UnsignedFixedPoint, Moment>> {
		let dia_key: Option<(Vec<u8>, Vec<u8>)> = ConvertKey::convert(key.clone());
		let Some((blockchain,symbol)) = dia_key else {
            return None;
        };

		let Ok(coin_info) = Dia::get_coin_info(blockchain, symbol) else {
            return None;
        };

		let Some(value) = ConvertPrice::convert(coin_info.price) else{
            return None;
        };
		let Some(timestamp) = ConvertMoment::convert(coin_info.last_update_timestamp) else{
            return None;
        };

		Some(TimestampedValue { value, timestamp })
	}

	fn get_all_values() -> Vec<(OracleKey, Option<TimestampedValue<UnsignedFixedPoint, Moment>>)> {
		// let r = T::get_coin_info(blockchain, symbol);
		todo!()
	}
}
