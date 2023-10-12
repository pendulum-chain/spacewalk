#[cfg(test)]
use mocktopus::macros::mockable;

#[cfg_attr(test, mockable)]
pub(crate) mod security {
	use frame_support::dispatch::DispatchResult;
	use sp_runtime::DispatchError;

	pub fn ensure_parachain_status_running<T: crate::Config>() -> DispatchResult {
		<security::Pallet<T>>::ensure_parachain_status_running()
	}

	pub fn parachain_block_expired<T: crate::Config>(
		opentime: T::BlockNumber,
		period: T::BlockNumber,
	) -> Result<bool, DispatchError> {
		<security::Pallet<T>>::parachain_block_expired(opentime, period)
	}
}
