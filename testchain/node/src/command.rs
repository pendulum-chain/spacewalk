// Copyright 2017-2020 Parity Technologies (UK) Ltd.
// This file is part of Substrate.

// Substrate is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Substrate is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Substrate.  If not, see <http://www.gnu.org/licenses/>.

use std::io::Write;

use frame_benchmarking_cli::{BenchmarkCmd, SUBSTRATE_REFERENCE_HARDWARE};
use sc_cli::{ChainSpec, Result, RuntimeVersion, SubstrateCli};
use sc_service::{Configuration, PartialComponents, TaskManager};
use sp_core::hexdisplay::HexDisplay;

use spacewalk_runtime::Block;

use crate::{
	chain_spec,
	cli::{Cli, Subcommand},
	service as spacewalk_service,
};

fn load_spec(id: &str) -> std::result::Result<Box<dyn sc_service::ChainSpec>, String> {
	match id {
		"" => Ok(Box::new(chain_spec::local_config())),
		"dev" => Ok(Box::new(chain_spec::development_config())),
		"beta" => Ok(Box::new(chain_spec::beta_testnet_config())),
		"testnet" => Ok(Box::new(chain_spec::ChainSpec::from_json_bytes(
			&include_bytes!("../res/testnet.json")[..],
		)?)),
		path => Ok(Box::new(chain_spec::ChainSpec::from_json_file(path.into())?)),
	}
}

impl SubstrateCli for Cli {
	fn impl_name() -> String {
		"spacewalk Parachain".into()
	}

	fn impl_version() -> String {
		env!("SUBSTRATE_CLI_IMPL_VERSION").into()
	}

	fn description() -> String {
		env!("CARGO_PKG_DESCRIPTION").into()
	}

	fn author() -> String {
		env!("CARGO_PKG_AUTHORS").into()
	}

	fn support_url() -> String {
		"https://github.com/interlay/spacewalk/issues/new".into()
	}

	fn copyright_start_year() -> i32 {
		2017
	}

	fn load_spec(&self, id: &str) -> std::result::Result<Box<dyn sc_service::ChainSpec>, String> {
		load_spec(id)
	}

	fn native_runtime_version(_: &Box<dyn ChainSpec>) -> &'static RuntimeVersion {
		&spacewalk_runtime::VERSION
	}
}

/// Parse command line arguments into service configuration.
pub fn run() -> Result<()> {
	let cli = Cli::from_args();

	match &cli.subcommand {
		Some(Subcommand::BuildSpec(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			runner.sync_run(|config| cmd.run(config.chain_spec, config.network))
		},
		Some(Subcommand::CheckBlock(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			runner.async_run(|config| {
				let PartialComponents { client, task_manager, import_queue, .. } =
					spacewalk_service::new_partial(&config)?;
				Ok((cmd.run(client, import_queue), task_manager))
			})
		},
		Some(Subcommand::ExportBlocks(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			runner.async_run(|config| {
				let PartialComponents { client, task_manager, .. } =
					spacewalk_service::new_partial(&config)?;
				Ok((cmd.run(client, config.database), task_manager))
			})
		},
		Some(Subcommand::ExportState(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			runner.async_run(|config| {
				let PartialComponents { client, task_manager, .. } =
					spacewalk_service::new_partial(&config)?;
				Ok((cmd.run(client, config.chain_spec), task_manager))
			})
		},
		Some(Subcommand::ImportBlocks(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			runner.async_run(|config| {
				let PartialComponents { client, task_manager, import_queue, .. } =
					spacewalk_service::new_partial(&config)?;
				Ok((cmd.run(client, import_queue), task_manager))
			})
		},
		Some(Subcommand::PurgeChain(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			runner.sync_run(|config| cmd.run(config.database))
		},
		Some(Subcommand::Revert(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			runner.async_run(|config| {
				let PartialComponents { client, task_manager, backend, .. } =
					spacewalk_service::new_partial(&config)?;
				let aux_revert = Box::new(|client, _, blocks| {
					sc_finality_grandpa::revert(client, blocks)?;
					Ok(())
				});
				Ok((cmd.run(client, backend, Some(aux_revert)), task_manager))
			})
		},
		Some(Subcommand::Benchmark(cmd)) => {
			let runner = cli.create_runner(cmd)?;

			runner.sync_run(|config| {
				// This switch needs to be in the client, since the client decides
				// which sub-commands it wants to support.
				match cmd {
					BenchmarkCmd::Pallet(cmd) =>
						if cfg!(feature = "runtime-benchmarks") {
							cmd.run::<Block, spacewalk_service::Executor>(config)
						} else {
							Err("Benchmarking wasn't enabled when building the node. \
                You can enable it with `--features runtime-benchmarks`."
								.into())
						},
					BenchmarkCmd::Block(cmd) => {
						let PartialComponents { client, .. } =
							spacewalk_service::new_partial(&config)?;
						cmd.run(client)
					},
					BenchmarkCmd::Storage(cmd) => {
						let PartialComponents { client, backend, .. } =
							spacewalk_service::new_partial(&config)?;
						let db = backend.expose_db();
						let storage = backend.expose_storage();

						cmd.run(config, client, db, storage)
					},
					BenchmarkCmd::Overhead(_) => Err("Unsupported benchmarking command".into()),
					BenchmarkCmd::Machine(cmd) =>
						cmd.run(&config, SUBSTRATE_REFERENCE_HARDWARE.clone()),
				}
			})
		},
		Some(Subcommand::ExportMetadata(params)) => {
			let mut ext = frame_support::BasicExternalities::default();
			sc_executor::with_externalities_safe(&mut ext, move || {
				let raw_meta_blob = spacewalk_runtime::Runtime::metadata().into();
				let output_buf = if params.raw {
					raw_meta_blob
				} else {
					format!("0x{:?}", HexDisplay::from(&raw_meta_blob)).into_bytes()
				};

				if let Some(output) = &params.output {
					std::fs::write(output, output_buf)?;
				} else {
					std::io::stdout().write_all(&output_buf)?;
				}

				Ok::<_, sc_cli::Error>(())
			})
			.map_err(|err| sc_cli::Error::Application(err.into()))??;

			Ok(())
		},
		None => {
			let runner = cli.create_runner(&*cli.run)?;

			runner
				.run_node_until_exit(|config| async move { start_node(cli, config).await })
				.map_err(Into::into)
		},
	}
}

async fn start_node(_: Cli, config: Configuration) -> sc_service::error::Result<TaskManager> {
	spacewalk_service::new_full(config).map(|(task_manager, _)| task_manager)
}
