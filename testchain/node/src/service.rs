use std::{sync::Arc, time::Duration};

use futures::{FutureExt, StreamExt};
use sc_client_api::{Backend, BlockBackend};
use sc_consensus_aura::{ImportQueueParams, SlotProportion, StartAuraParams};
use sc_consensus_grandpa::SharedVoterState;
use sc_executor::{
	HeapAllocStrategy, NativeElseWasmExecutor, WasmExecutor, DEFAULT_HEAP_ALLOC_STRATEGY,
};
use sc_service::{
	error::Error as ServiceError, Configuration, RpcHandlers, TFullBackend, TFullClient,
	TaskManager,
};
use sc_telemetry::{Telemetry, TelemetryWorker};
use sc_transaction_pool_api::OffchainTransactionPoolFactory;
use sp_consensus_aura::sr25519::AuthorityPair as AuraPair;
use sp_core::crypto::KeyTypeId;

use crate::rpc as spacewalk_rpc;
use primitives::Block;

use spacewalk_runtime_mainnet::RuntimeApi as MainnetRuntimeApi;
use spacewalk_runtime_testnet::RuntimeApi as TestnetRuntimeApi;

// Native executor instance.
pub struct TestnetExecutor;

/// The minimum period of blocks on which justifications will be
/// imported and generated.
const GRANDPA_JUSTIFICATION_PERIOD: u32 = 512;

impl sc_executor::NativeExecutionDispatch for TestnetExecutor {
	/// Only enable the benchmarking host functions when we actually want to benchmark.
	#[cfg(feature = "runtime-benchmarks")]
	type ExtendHostFunctions = frame_benchmarking::benchmarking::HostFunctions;
	/// Otherwise we only use the default Substrate host functions.
	#[cfg(not(feature = "runtime-benchmarks"))]
	type ExtendHostFunctions = ();

	fn dispatch(method: &str, data: &[u8]) -> Option<Vec<u8>> {
		spacewalk_runtime_testnet::api::dispatch(method, data)
	}

	fn native_version() -> sc_executor::NativeVersion {
		spacewalk_runtime_testnet::native_version()
	}
}

pub struct MainnetExecutor;

impl sc_executor::NativeExecutionDispatch for MainnetExecutor {
	/// Only enable the benchmarking host functions when we actually want to benchmark.
	#[cfg(feature = "runtime-benchmarks")]
	type ExtendHostFunctions = frame_benchmarking::benchmarking::HostFunctions;
	/// Otherwise we only use the default Substrate host functions.
	#[cfg(not(feature = "runtime-benchmarks"))]
	type ExtendHostFunctions = ();

	fn dispatch(method: &str, data: &[u8]) -> Option<Vec<u8>> {
		spacewalk_runtime_mainnet::api::dispatch(method, data)
	}

	fn native_version() -> sc_executor::NativeVersion {
		spacewalk_runtime_mainnet::native_version()
	}
}

pub type FullTestnetClient =
	TFullClient<Block, TestnetRuntimeApi, NativeElseWasmExecutor<TestnetExecutor>>;

pub type FullMainnetClient =
	TFullClient<Block, MainnetRuntimeApi, NativeElseWasmExecutor<MainnetExecutor>>;

pub type FullBackend = TFullBackend<Block>;

type FullSelectChain = sc_consensus::LongestChain<FullBackend, Block>;

#[allow(clippy::type_complexity)]
#[allow(clippy::redundant_clone)]
pub fn new_partial_mainnet(
	config: &Configuration,
	instant_seal: bool,
) -> Result<
	sc_service::PartialComponents<
		FullMainnetClient,
		FullBackend,
		FullSelectChain,
		sc_consensus::DefaultImportQueue<Block>,
		sc_transaction_pool::FullPool<Block, FullMainnetClient>,
		(
			sc_consensus_grandpa::GrandpaBlockImport<
				FullBackend,
				Block,
				FullMainnetClient,
				FullSelectChain,
			>,
			sc_consensus_grandpa::LinkHalf<Block, FullMainnetClient, FullSelectChain>,
			Option<Telemetry>,
		),
	>,
	ServiceError,
> {
	let telemetry = config
		.telemetry_endpoints
		.clone()
		.filter(|x| !x.is_empty())
		.map(|endpoints| -> Result<_, sc_telemetry::Error> {
			let worker = TelemetryWorker::new(16)?;
			let telemetry = worker.handle().new_telemetry(endpoints);
			Ok((worker, telemetry))
		})
		.transpose()?;

	let heap_pages = config
		.default_heap_pages
		.map_or(DEFAULT_HEAP_ALLOC_STRATEGY, |h| HeapAllocStrategy::Static { extra_pages: h as _ });

	let wasm = WasmExecutor::builder()
		.with_execution_method(config.wasm_method)
		.with_onchain_heap_alloc_strategy(heap_pages)
		.with_offchain_heap_alloc_strategy(heap_pages)
		.with_max_runtime_instances(config.max_runtime_instances)
		.with_runtime_cache_size(config.runtime_cache_size)
		.build();

	let executor = NativeElseWasmExecutor::<MainnetExecutor>::new_with_wasm_executor(wasm);

	let (client, backend, keystore_container, task_manager) =
		sc_service::new_full_parts::<Block, MainnetRuntimeApi, _>(
			config,
			telemetry.as_ref().map(|(_, telemetry)| telemetry.handle()),
			executor,
		)?;
	let client = Arc::new(client);

	let telemetry = telemetry.map(|(worker, telemetry)| {
		task_manager.spawn_handle().spawn("telemetry", None, worker.run());
		telemetry
	});

	let select_chain = sc_consensus::LongestChain::new(backend.clone());

	let transaction_pool = sc_transaction_pool::BasicPool::new_full(
		config.transaction_pool.clone(),
		config.role.is_authority().into(),
		config.prometheus_registry(),
		task_manager.spawn_essential_handle(),
		client.clone(),
	);

	let (grandpa_block_import, grandpa_link) = sc_consensus_grandpa::block_import(
		client.clone(),
		GRANDPA_JUSTIFICATION_PERIOD,
		&(client.clone() as Arc<_>),
		select_chain.clone(),
		telemetry.as_ref().map(|x| x.handle()),
	)?;

	let slot_duration = sc_consensus_aura::slot_duration(&*client)?;
	let registry = config.prometheus_registry();

	let import_queue = if instant_seal {
		// instance sealing
		sc_consensus_manual_seal::import_queue(
			Box::new(client.clone()),
			&task_manager.spawn_essential_handle(),
			registry,
		)
	} else {
		sc_consensus_aura::import_queue::<AuraPair, _, _, _, _, _>(ImportQueueParams {
			block_import: grandpa_block_import.clone(),
			justification_import: Some(Box::new(grandpa_block_import.clone())),
			client: client.clone(),
			create_inherent_data_providers: move |_, ()| async move {
				let timestamp = sp_timestamp::InherentDataProvider::from_system_time();

				let slot =
					sp_consensus_aura::inherents::InherentDataProvider::from_timestamp_and_slot_duration(
						*timestamp,
						slot_duration,
					);

				Ok((slot, timestamp))
			},
			spawner: &task_manager.spawn_essential_handle(),
			registry,
			check_for_equivocation: Default::default(),
			telemetry: telemetry.as_ref().map(|x| x.handle()),
			compatibility_mode: Default::default(),
		})?
	};

	Ok(sc_service::PartialComponents {
		client,
		backend,
		task_manager,
		import_queue,
		keystore_container,
		select_chain,
		transaction_pool,
		other: (grandpa_block_import, grandpa_link, telemetry),
	})
}

#[allow(clippy::type_complexity)]
#[allow(clippy::redundant_clone)]
pub fn new_partial_testnet(
	config: &Configuration,
	instant_seal: bool,
) -> Result<
	sc_service::PartialComponents<
		FullTestnetClient,
		FullBackend,
		FullSelectChain,
		sc_consensus::DefaultImportQueue<Block>,
		sc_transaction_pool::FullPool<Block, FullTestnetClient>,
		(
			sc_consensus_grandpa::GrandpaBlockImport<
				FullBackend,
				Block,
				FullTestnetClient,
				FullSelectChain,
			>,
			sc_consensus_grandpa::LinkHalf<Block, FullTestnetClient, FullSelectChain>,
			Option<Telemetry>,
		),
	>,
	ServiceError,
> {
	let telemetry = config
		.telemetry_endpoints
		.clone()
		.filter(|x| !x.is_empty())
		.map(|endpoints| -> Result<_, sc_telemetry::Error> {
			let worker = TelemetryWorker::new(16)?;
			let telemetry = worker.handle().new_telemetry(endpoints);
			Ok((worker, telemetry))
		})
		.transpose()?;

	let heap_pages = config
		.default_heap_pages
		.map_or(DEFAULT_HEAP_ALLOC_STRATEGY, |h| HeapAllocStrategy::Static { extra_pages: h as _ });

	let wasm = WasmExecutor::builder()
		.with_execution_method(config.wasm_method)
		.with_onchain_heap_alloc_strategy(heap_pages)
		.with_offchain_heap_alloc_strategy(heap_pages)
		.with_max_runtime_instances(config.max_runtime_instances)
		.with_runtime_cache_size(config.runtime_cache_size)
		.build();

	let executor = NativeElseWasmExecutor::<TestnetExecutor>::new_with_wasm_executor(wasm);

	let (client, backend, keystore_container, task_manager) =
		sc_service::new_full_parts::<Block, TestnetRuntimeApi, _>(
			config,
			telemetry.as_ref().map(|(_, telemetry)| telemetry.handle()),
			executor,
		)?;
	let client = Arc::new(client);

	let telemetry = telemetry.map(|(worker, telemetry)| {
		task_manager.spawn_handle().spawn("telemetry", None, worker.run());
		telemetry
	});

	let select_chain = sc_consensus::LongestChain::new(backend.clone());

	let transaction_pool = sc_transaction_pool::BasicPool::new_full(
		config.transaction_pool.clone(),
		config.role.is_authority().into(),
		config.prometheus_registry(),
		task_manager.spawn_essential_handle(),
		client.clone(),
	);

	let (grandpa_block_import, grandpa_link) = sc_consensus_grandpa::block_import(
		client.clone(),
		GRANDPA_JUSTIFICATION_PERIOD,
		&(client.clone() as Arc<_>),
		select_chain.clone(),
		telemetry.as_ref().map(|x| x.handle()),
	)?;

	let slot_duration = sc_consensus_aura::slot_duration(&*client)?;
	let registry = config.prometheus_registry();

	let import_queue = if instant_seal {
		// instance sealing
		sc_consensus_manual_seal::import_queue(
			Box::new(client.clone()),
			&task_manager.spawn_essential_handle(),
			registry,
		)
	} else {
		sc_consensus_aura::import_queue::<AuraPair, _, _, _, _, _>(ImportQueueParams {
			block_import: grandpa_block_import.clone(),
			justification_import: Some(Box::new(grandpa_block_import.clone())),
			client: client.clone(),
			create_inherent_data_providers: move |_, ()| async move {
				let timestamp = sp_timestamp::InherentDataProvider::from_system_time();

				let slot =
					sp_consensus_aura::inherents::InherentDataProvider::from_timestamp_and_slot_duration(
						*timestamp,
						slot_duration,
					);

				Ok((slot, timestamp))
			},
			spawner: &task_manager.spawn_essential_handle(),
			registry,
			check_for_equivocation: Default::default(),
			telemetry: telemetry.as_ref().map(|x| x.handle()),
			compatibility_mode: Default::default(),
		})?
	};

	Ok(sc_service::PartialComponents {
		client,
		backend,
		task_manager,
		import_queue,
		keystore_container,
		select_chain,
		transaction_pool,
		other: (grandpa_block_import, grandpa_link, telemetry),
	})
}

/// Builds a new service for a full client.
pub fn new_full(config: Configuration) -> Result<(TaskManager, RpcHandlers), ServiceError> {
	let sc_service::PartialComponents {
		client,
		backend,
		mut task_manager,
		import_queue,
		keystore_container,
		select_chain,
		transaction_pool,
		other: (block_import, grandpa_link, mut telemetry),
	} = new_partial_mainnet(&config, false)?;

	let grandpa_protocol_name = sc_consensus_grandpa::protocol_standard_name(
		&client.block_hash(0).ok().flatten().expect("Genesis block exists; qed"),
		&config.chain_spec,
	);

	let mut net_config = sc_network::config::FullNetworkConfiguration::new(&config.network);

	net_config.add_notification_protocol(sc_consensus_grandpa::grandpa_peers_set_config(
		grandpa_protocol_name.clone(),
	));

	let (network, system_rpc_tx, tx_handler_controller, network_starter, sync_service) =
		sc_service::build_network(sc_service::BuildNetworkParams {
			config: &config,
			net_config,
			client: client.clone(),
			transaction_pool: transaction_pool.clone(),
			spawn_handle: task_manager.spawn_handle(),
			import_queue,
			block_announce_validator_builder: None,
			warp_sync_params: None,
		})?;

	if config.offchain_worker.enabled {
		// Initialize seed for signing transaction using off-chain workers. This is a convenience
		// so learners can see the transactions submitted simply running the node.
		// Typically these keys should be inserted with RPC calls to `author_insertKey`.
		let keystore = keystore_container.keystore();
		// Note that the `KeyTypeId` here is very important and the content must match the one
		// defined in the DIA oracle pallet. Otherwise the signer key is not available for the
		// off-chain worker of the pallet.
		pub const KEY_TYPE: KeyTypeId = KeyTypeId(*b"dia!");
		sp_keystore::Keystore::sr25519_generate_new(&*keystore, KEY_TYPE, Some("//Bob"))
			.expect("Creating key with account Bob should succeed.");

		task_manager.spawn_handle().spawn(
			"offchain-workers-runner",
			"offchain-worker",
			sc_offchain::OffchainWorkers::new(sc_offchain::OffchainWorkerOptions {
				runtime_api_provider: client.clone(),
				is_validator: config.role.is_authority(),
				keystore: Some(keystore),
				offchain_db: backend.offchain_storage(),
				transaction_pool: Some(OffchainTransactionPoolFactory::new(
					transaction_pool.clone(),
				)),
				network_provider: network.clone(),
				enable_http_requests: true,
				custom_extensions: |_| vec![],
			})
			.run(client.clone(), task_manager.spawn_handle())
			.boxed(),
		);
	}

	let role = config.role.clone();
	let force_authoring = config.force_authoring;
	let backoff_authoring_blocks: Option<()> = None;
	let name = config.network.node_name.clone();
	let enable_grandpa = !config.disable_grandpa;
	let prometheus_registry = config.prometheus_registry().cloned();

	let rpc_builder = {
		let client = client.clone();
		let pool = transaction_pool.clone();

		Box::new(move |deny_unsafe, _| {
			let deps = spacewalk_rpc::FullDeps {
				client: client.clone(),
				pool: pool.clone(),
				deny_unsafe,
				command_sink: None,
			};

			spacewalk_rpc::create_full(deps).map_err(Into::into)
		})
	};

	let rpc_handlers = sc_service::spawn_tasks(sc_service::SpawnTasksParams {
		network: network.clone(),
		client: client.clone(),
		keystore: keystore_container.keystore(),
		task_manager: &mut task_manager,
		transaction_pool: transaction_pool.clone(),
		rpc_builder,
		backend,
		system_rpc_tx,
		tx_handler_controller,
		config,
		telemetry: telemetry.as_mut(),
		sync_service: sync_service.clone(),
	})?;

	if role.is_authority() {
		let proposer_factory = sc_basic_authorship::ProposerFactory::new(
			task_manager.spawn_handle(),
			client.clone(),
			transaction_pool.clone(),
			prometheus_registry.as_ref(),
			telemetry.as_ref().map(|x| x.handle()),
		);

		let slot_duration = sc_consensus_aura::slot_duration(&*client)?;

		let aura = sc_consensus_aura::start_aura::<AuraPair, _, _, _, _, _, _, _, _, _, _>(
			StartAuraParams {
				slot_duration,
				client,
				select_chain,
				block_import,
				proposer_factory,
				create_inherent_data_providers: move |_, ()| async move {
					let timestamp = sp_timestamp::InherentDataProvider::from_system_time();

					let slot =
						sp_consensus_aura::inherents::InherentDataProvider::from_timestamp_and_slot_duration(
							*timestamp,
							slot_duration,
						);

					Ok((slot, timestamp))
				},
				force_authoring,
				backoff_authoring_blocks,
				keystore: keystore_container.keystore(),
				sync_oracle: sync_service.clone(),
				justification_sync_link: sync_service.clone(),
				block_proposal_slot_portion: SlotProportion::new(2f32 / 3f32),
				max_block_proposal_slot_portion: None,
				telemetry: telemetry.as_ref().map(|x| x.handle()),
				compatibility_mode: Default::default(),
			},
		)?;

		// the AURA authoring task is considered essential, i.e. if it
		// fails we take down the service with it.
		task_manager.spawn_essential_handle().spawn_blocking("aura", None, aura);
	}

	// if the node isn't actively participating in consensus then it doesn't
	// need a keystore, regardless of which protocol we use below.
	let keystore = if role.is_authority() { Some(keystore_container.keystore()) } else { None };

	let grandpa_config = sc_consensus_grandpa::Config {
		// FIXME #1578 make this available through chainspec
		gossip_duration: Duration::from_millis(333),
		name: Some(name),
		observer_enabled: false,
		keystore,
		local_role: role,
		telemetry: telemetry.as_ref().map(|x| x.handle()),
		protocol_name: grandpa_protocol_name,
		justification_generation_period: GRANDPA_JUSTIFICATION_PERIOD,
	};

	if enable_grandpa {
		// start the full GRANDPA voter
		// NOTE: non-authorities could run the GRANDPA observer protocol, but at
		// this point the full voter should provide better guarantees of block
		// and vote data availability than the observer. The observer has not
		// been tested extensively yet and having most nodes in a network run it
		// could lead to finality stalls.
		let grandpa_config = sc_consensus_grandpa::GrandpaParams {
			config: grandpa_config,
			link: grandpa_link,
			network,
			sync: Arc::new(sync_service),
			voting_rule: sc_consensus_grandpa::VotingRulesBuilder::default().build(),
			prometheus_registry,
			shared_voter_state: SharedVoterState::empty(),
			telemetry: telemetry.as_ref().map(|x| x.handle()),
			offchain_tx_pool_factory: OffchainTransactionPoolFactory::new(transaction_pool),
		};

		// the GRANDPA voter task is considered infallible, i.e.
		// if it fails we take down the service with it.
		task_manager.spawn_essential_handle().spawn_blocking(
			"grandpa-voter",
			None,
			sc_consensus_grandpa::run_grandpa_voter(grandpa_config)?,
		);
	}

	network_starter.start_network();
	Ok((task_manager, rpc_handlers))
}

#[allow(dead_code)]
pub async fn start_instant_mainnet(
	config: Configuration,
) -> sc_service::error::Result<(TaskManager, RpcHandlers)> {
	let sc_service::PartialComponents {
		client,
		backend,
		mut task_manager,
		import_queue,
		keystore_container,
		select_chain,
		transaction_pool,
		other: (_, _, mut telemetry),
	} = new_partial_mainnet(&config, true)?;

	let net_config = sc_network::config::FullNetworkConfiguration::new(&config.network);

	let (network, system_rpc_tx, tx_handler_controller, network_starter, sync_service) =
		sc_service::build_network(sc_service::BuildNetworkParams {
			config: &config,
			net_config,
			client: client.clone(),
			transaction_pool: transaction_pool.clone(),
			spawn_handle: task_manager.spawn_handle(),
			import_queue,
			block_announce_validator_builder: None,
			warp_sync_params: None,
		})?;

	if config.offchain_worker.enabled {
		task_manager.spawn_handle().spawn(
			"offchain-workers-runner",
			"offchain-worker",
			sc_offchain::OffchainWorkers::new(sc_offchain::OffchainWorkerOptions {
				runtime_api_provider: client.clone(),
				is_validator: config.role.is_authority(),
				keystore: Some(keystore_container.keystore()),
				offchain_db: backend.offchain_storage(),
				transaction_pool: Some(OffchainTransactionPoolFactory::new(
					transaction_pool.clone(),
				)),
				network_provider: network.clone(),
				enable_http_requests: true,
				custom_extensions: |_| vec![],
			})
			.run(client.clone(), task_manager.spawn_handle())
			.boxed(),
		);
	};

	let prometheus_registry = config.prometheus_registry().cloned();

	let role = config.role.clone();

	let command_sink = if role.is_authority() {
		let proposer_factory = sc_basic_authorship::ProposerFactory::new(
			task_manager.spawn_handle(),
			client.clone(),
			transaction_pool.clone(),
			prometheus_registry.as_ref(),
			telemetry.as_ref().map(|x| x.handle()),
		);

		// Channel for the rpc handler to communicate with the authorship task.
		let (command_sink, commands_stream) = futures::channel::mpsc::channel(1024);

		let pool = transaction_pool.pool().clone();
		let import_stream = pool.validated_pool().import_notification_stream().map(|_| {
			sc_consensus_manual_seal::rpc::EngineCommand::SealNewBlock {
				create_empty: true,
				finalize: true,
				parent_hash: None,
				sender: None,
			}
		});

		let authorship_future =
			sc_consensus_manual_seal::run_manual_seal(sc_consensus_manual_seal::ManualSealParams {
				block_import: client.clone(),
				env: proposer_factory,
				client: client.clone(),
				pool: transaction_pool.clone(),
				commands_stream: futures::stream_select!(commands_stream, import_stream),
				select_chain,
				consensus_data_provider: None,
				create_inherent_data_providers: move |_, ()| async move {
					Ok(sp_timestamp::InherentDataProvider::from_system_time())
				},
			});

		// we spawn the future on a background thread managed by service.
		task_manager.spawn_essential_handle().spawn_blocking(
			"instant-seal",
			Some("block-authoring"),
			authorship_future,
		);
		Some(command_sink)
	} else {
		None
	};

	let rpc_builder = {
		let client = client.clone();
		let transaction_pool = transaction_pool.clone();

		move |deny_unsafe, _| {
			let deps = spacewalk_rpc::FullDeps {
				client: client.clone(),
				pool: transaction_pool.clone(),
				deny_unsafe,
				command_sink: command_sink.clone(),
			};

			spacewalk_rpc::create_full(deps).map_err(Into::into)
		}
	};

	let rpc_handlers = sc_service::spawn_tasks(sc_service::SpawnTasksParams {
		network,
		client,
		keystore: keystore_container.keystore(),
		task_manager: &mut task_manager,
		transaction_pool,
		rpc_builder: Box::new(rpc_builder),
		backend,
		system_rpc_tx,
		tx_handler_controller,
		config,
		telemetry: telemetry.as_mut(),
		sync_service,
	})?;

	network_starter.start_network();

	Ok((task_manager, rpc_handlers))
}

#[allow(dead_code)]
pub async fn start_instant_testnet(
	config: Configuration,
) -> sc_service::error::Result<(TaskManager, RpcHandlers)> {
	let sc_service::PartialComponents {
		client,
		backend,
		mut task_manager,
		import_queue,
		keystore_container,
		select_chain,
		transaction_pool,
		other: (_, _, mut telemetry),
	} = new_partial_testnet(&config, true)?;

	let net_config = sc_network::config::FullNetworkConfiguration::new(&config.network);

	let (network, system_rpc_tx, tx_handler_controller, network_starter, sync_service) =
		sc_service::build_network(sc_service::BuildNetworkParams {
			config: &config,
			net_config,
			client: client.clone(),
			transaction_pool: transaction_pool.clone(),
			spawn_handle: task_manager.spawn_handle(),
			import_queue,
			block_announce_validator_builder: None,
			warp_sync_params: None,
		})?;

	if config.offchain_worker.enabled {
		task_manager.spawn_handle().spawn(
			"offchain-workers-runner",
			"offchain-worker",
			sc_offchain::OffchainWorkers::new(sc_offchain::OffchainWorkerOptions {
				runtime_api_provider: client.clone(),
				is_validator: config.role.is_authority(),
				keystore: Some(keystore_container.keystore()),
				offchain_db: backend.offchain_storage(),
				transaction_pool: Some(OffchainTransactionPoolFactory::new(
					transaction_pool.clone(),
				)),
				network_provider: network.clone(),
				enable_http_requests: true,
				custom_extensions: |_| vec![],
			})
			.run(client.clone(), task_manager.spawn_handle())
			.boxed(),
		);
	};

	let prometheus_registry = config.prometheus_registry().cloned();

	let role = config.role.clone();

	let command_sink = if role.is_authority() {
		let proposer_factory = sc_basic_authorship::ProposerFactory::new(
			task_manager.spawn_handle(),
			client.clone(),
			transaction_pool.clone(),
			prometheus_registry.as_ref(),
			telemetry.as_ref().map(|x| x.handle()),
		);

		// Channel for the rpc handler to communicate with the authorship task.
		let (command_sink, commands_stream) = futures::channel::mpsc::channel(1024);

		let pool = transaction_pool.pool().clone();
		let import_stream = pool.validated_pool().import_notification_stream().map(|_| {
			sc_consensus_manual_seal::rpc::EngineCommand::SealNewBlock {
				create_empty: true,
				finalize: true,
				parent_hash: None,
				sender: None,
			}
		});

		let authorship_future =
			sc_consensus_manual_seal::run_manual_seal(sc_consensus_manual_seal::ManualSealParams {
				block_import: client.clone(),
				env: proposer_factory,
				client: client.clone(),
				pool: transaction_pool.clone(),
				commands_stream: futures::stream_select!(commands_stream, import_stream),
				select_chain,
				consensus_data_provider: None,
				create_inherent_data_providers: move |_, ()| async move {
					Ok(sp_timestamp::InherentDataProvider::from_system_time())
				},
			});

		// we spawn the future on a background thread managed by service.
		task_manager.spawn_essential_handle().spawn_blocking(
			"instant-seal",
			Some("block-authoring"),
			authorship_future,
		);
		Some(command_sink)
	} else {
		None
	};

	let rpc_builder = {
		let client = client.clone();
		let transaction_pool = transaction_pool.clone();

		move |deny_unsafe, _| {
			let deps = spacewalk_rpc::FullDeps {
				client: client.clone(),
				pool: transaction_pool.clone(),
				deny_unsafe,
				command_sink: command_sink.clone(),
			};

			spacewalk_rpc::create_full(deps).map_err(Into::into)
		}
	};

	let rpc_handlers = sc_service::spawn_tasks(sc_service::SpawnTasksParams {
		network,
		client,
		keystore: keystore_container.keystore(),
		task_manager: &mut task_manager,
		transaction_pool,
		rpc_builder: Box::new(rpc_builder),
		backend,
		system_rpc_tx,
		tx_handler_controller,
		config,
		telemetry: telemetry.as_mut(),
		sync_service,
	})?;

	network_starter.start_network();

	Ok((task_manager, rpc_handlers))
}
