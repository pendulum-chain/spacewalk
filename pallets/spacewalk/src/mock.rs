pub use spacewalk_runtime_standalone::*;

// Build genesis storage according to the mock runtime.
pub fn new_test_ext() -> sp_io::TestExternalities {
	frame_system::GenesisConfig::default()
		.build_storage::<Runtime>()
		.unwrap()
		.into()
}
