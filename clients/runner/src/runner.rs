use crate::{error::Error, Opts};
use bytes::Bytes;
use codec::Decode;
use futures::{future::BoxFuture, FutureExt, StreamExt, TryFutureExt};
use nix::{
	sys::signal::{self, Signal},
	unistd::Pid,
};
use reqwest::Url;
use sha2::{Digest, Sha256};
use signal_hook_tokio::SignalsInfo;
use sp_core::{hexdisplay::AsBytesRef, H256};
use std::{
	convert::TryInto,
	fmt::{Debug, Display},
	fs::{self, OpenOptions},
	io::{Read, Seek, Write},
	os::unix::prelude::OpenOptionsExt,
	path::PathBuf,
	process::{Child, Command, Stdio},
	str::{self, FromStr},
	time::Duration,
};

use subxt::{dynamic::Value, OnlineClient, PolkadotConfig};

use async_trait::async_trait;

/// Type of the client to run.
/// Also used as the name of the downloaded executable.
#[derive(Debug, Clone)]
pub enum ClientType {
	Vault,
}

impl FromStr for ClientType {
	type Err = crate::Error;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		let client_type = match s {
			"vault" => ClientType::Vault,
			_ => return Err(Error::ClientTypeParsingError),
		};
		Ok(client_type)
	}
}

impl Display for ClientType {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		let s = match self {
			ClientType::Vault => "vault",
		};
		write!(f, "{s}")
	}
}

/// Pallet in the parachain where the client release is assumed to be stored
pub const PARACHAIN_MODULE: &str = "ClientsInfo";

/// Storage item in the Pallet where the client release is assumed to be stored
pub const CURRENT_RELEASES_STORAGE_ITEM: &str = "CurrentClientReleases";

/// Parachain block time
pub const BLOCK_TIME: Duration = Duration::from_secs(12);

/// The duration up until operations are retried, used by the retry utilities: 60 seconds
pub const RETRY_TIMEOUT: Duration = Duration::from_millis(60_000);

/// Waiting (sleep) interval used by the retry utilities: 1 second
pub const RETRY_INTERVAL: Duration = Duration::from_millis(1_000);

/// Data type assumed to be used by the parachain to store the client release.
/// If this type is different from the on-chain one, decoding will fail.
#[derive(Decode, Default, Eq, PartialEq, Debug, Clone)]
pub struct ClientRelease {
	/// A link to the Releases page of the `interbtc-clients` repo.
	/// Example: https://downloads.pendulumchain.tech/spacewalk/vault-rococo
	pub uri: String,
	/// The SHA256 checksum of the client binary.
	pub checksum: H256,
}

/// Wrapper around `ClientRelease`, which includes details for running the executable.
#[derive(Default, Eq, PartialEq, Debug, Clone)]
pub struct DownloadedRelease {
	/// The SHA256 checksum of the client binary.
	pub checksum: H256,
	/// OS path where this release is stored
	pub path: PathBuf,
	/// Name of the executable
	pub bin_name: String,
}

fn sha256sum(bytes: &[u8]) -> Vec<u8> {
	let mut hasher = Sha256::default();
	hasher.input(bytes);
	hasher.result().as_slice().to_vec()
}

/// Per-network manager of the client executable
pub struct Runner {
	/// `subxt` api to the parachain
	subxt_api: OnlineClient<PolkadotConfig>,
	/// The child process (spacewalk vault client) spawned by this runner
	child_proc: Option<Child>,
	/// Details about the currently run release
	downloaded_release: Option<DownloadedRelease>,
	/// Runner CLI arguments
	opts: Opts,
}

impl Runner {
	pub fn new(subxt_api: OnlineClient<PolkadotConfig>, opts: Opts) -> Self {
		Self { subxt_api, child_proc: None, downloaded_release: None, opts }
	}

	fn try_load_downloaded_binary(
		runner: &mut impl RunnerExt,
		release: &ClientRelease,
	) -> Result<(), Error> {
		let (bin_name, bin_path) = runner.get_bin_path(&release.uri)?;
		let file_content = fs::read(bin_path.clone()).map_err(|_| Error::NoDownloadedRelease)?;
		let checksum = H256::from_slice(&sha256sum(&file_content));

		runner.checksum_matches(checksum, release)?;

		let downloaded_release = DownloadedRelease { checksum, path: bin_path.clone(), bin_name };

		runner.set_downloaded_release(Some(downloaded_release));
		if let Ok(stringified_path) = bin_path.into_os_string().into_string() {
			log::info!(
				"Loaded binary from disk, at {}, with checksum {:#x}",
				stringified_path,
				checksum
			);
		}
		Ok(())
	}

	async fn download_binary(
		runner: &mut impl RunnerExt,
		release: ClientRelease,
	) -> Result<DownloadedRelease, Error> {
		//recheck checksum to see if binary is the one we have
		if let Some(downloaded_release) = runner.downloaded_release() {
			if runner.checksum_matches(downloaded_release.checksum, &release).is_ok() {
				log::info!("The on-chain release is already downloaded, skipping re-download");
				return Ok(downloaded_release.clone())
			}
		}

		let (bin_name, bin_path) = runner.get_bin_path(&release.uri)?;
		log::info!("Downloading {} at: {:?}", bin_name, bin_path);

		// if not, try get the binary
		let checksum = retry_with_log_async(
			|| {
				Runner::do_download_binary(bin_path.clone(), release.clone())
					.into_future()
					.boxed()
			},
			"Error downloading executable".to_string(),
		)
		.await?;

		runner.checksum_matches(checksum, &release)?;

		let downloaded_release =
			DownloadedRelease { checksum: release.checksum, path: bin_path, bin_name };
		runner.set_downloaded_release(Some(downloaded_release.clone()));
		Ok(downloaded_release)
	}

	async fn do_download_binary(bin_path: PathBuf, release: ClientRelease) -> Result<H256, Error> {
		let mut file = OpenOptions::new()
			.read(true)
			.write(true)
			// Make the binary executable.
			// The set permissions are: -rwx------
			.mode(0o700)
			.create(true)
			.truncate(true)
			.open(bin_path.clone())?;

		let bytes = retry_with_log_async(
			|| Runner::get_request_bytes(release.uri.clone()).into_future().boxed(),
			"Error getting request bytes for executable".to_string(),
		)
		.await?;

		file.write_all(&bytes)?;
		file.sync_all()?;

		//checksum check
		file.rewind()?;

		// Read the content of the file
		let mut file_content = Vec::new();
		file.read_to_end(&mut file_content)?;

		Ok(H256::from_slice(&sha256sum(&file_content)))
	}

	async fn get_request_bytes(url: String) -> Result<Bytes, Error> {
		log::info!("Fetching executable from {}", url);
		let response = reqwest::get(url.clone()).await?;
		Ok(response.bytes().await?)
	}

	fn checksum_matches(checksum: H256, client_release: &ClientRelease) -> Result<(), Error> {
		if checksum != client_release.checksum {
			return Err(Error::IncorrectChecksum)
		}

		Ok(())
	}

	fn get_bin_path(runner: &impl RunnerExt, uri: &str) -> Result<(String, PathBuf), Error> {
		// Remove any trailing slashes from the release URI
		let parsed_uri = Url::parse(uri.trim_end_matches('/'))?;
		let bin_name = parsed_uri
			.path_segments()
			.and_then(|segments| segments.last())
			.and_then(|name| if name.is_empty() { None } else { Some(name) })
			.ok_or(Error::ClientNameDerivationError)?;

		let bin_path = runner.download_path().join(bin_name);
		Ok((bin_name.to_string(), bin_path))
	}

	fn delete_downloaded_release(runner: &mut impl RunnerExt) -> Result<(), Error> {
		let release = runner.downloaded_release().as_ref().ok_or(Error::NoDownloadedRelease)?;
		log::info!("Removing old release, with path {:?}", release.path);

		retry_with_log(
			|| Ok(fs::remove_file(&release.path)?),
			"Failed to remove old release".to_string(),
		)?;

		runner.set_downloaded_release(None);
		Ok(())
	}

	fn terminate_proc_and_wait(runner: &mut impl RunnerExt) -> Result<u32, Error> {
		log::info!("Trying to terminate child process...");
		let child_proc = match runner.child_proc().as_mut() {
			Some(x) => x,
			None => {
				log::warn!("No child process to terminate.");
				return Ok(0)
			},
		};

		let _ = retry_with_log(
			|| {
				Ok(signal::kill(
					Pid::from_raw(
						child_proc.id().try_into().map_err(|_| Error::IntegerConversionError)?,
					),
					Signal::SIGTERM,
				))
			},
			"Failed to kill child process".to_string(),
		)
		.map_err(|_| Error::ProcessTerminationFailure)?;

		match child_proc.wait() {
			Ok(exit_status) => log::info!(
				"Terminated client process (pid: {}) with exit status {}",
				child_proc.id(),
				exit_status
			),
			Err(error) => log::warn!("Client process termination error: {}", error),
		};
		let pid = child_proc.id();
		runner.set_child_proc(None);
		Ok(pid)
	}

	async fn try_get_release<T: RunnerExt + StorageReader>(
		runner: &T,
	) -> Result<Option<ClientRelease>, Error> {
		retry_with_log_async(
			|| {
				runner
					.read_chain_storage::<ClientRelease>(runner.subxt_api())
					.into_future()
					.boxed()
			},
			"Error reading chain storage for release".to_string(),
		)
		.await
	}

	/// Read parachain storage via an RPC call, and decode the result
	async fn read_chain_storage<T: 'static + Decode + Debug>(
		client_type: &ClientType,
		subxt_api: &OnlineClient<PolkadotConfig>,
	) -> Result<Option<T>, Error> {
		// Based on the implementation of `subxt_api.storage().fetch(...)`, but with decoding for a
		// custom type. Source: https://github.com/paritytech/subxt/blob/99cea97f817ee0a6fee642ff22f867822d9557f6/subxt/src/storage/storage_client.rs#L142
		let storage_address = subxt::dynamic::storage(
			PARACHAIN_MODULE,
			CURRENT_RELEASES_STORAGE_ITEM,
			vec![Value::from_bytes(client_type.to_string().as_bytes())],
		);
		let lookup_bytes = subxt_api.storage().address_bytes(&storage_address)?;
		let enc_res = subxt_api
			.storage()
			.at_latest()
			.await?
			.fetch_raw(&*lookup_bytes)
			.await?
			.map(Bytes::from);

		enc_res
			.map(|r| T::decode(&mut r.as_bytes_ref()))
			.transpose()
			.map_err(Into::into)
	}

	/// Run the auto-updater while concurrently listening for termination signals.
	pub async fn run(
		mut runner: Box<dyn RunnerExt + Send>,
		mut shutdown_signals: SignalsInfo,
	) -> Result<(), Error> {
		tokio::select! {
			_ = shutdown_signals.next() => {
				runner.terminate_proc_and_wait()?;
			}
			result = runner.as_mut().auto_update() => {
				match result {
					Ok(_) => log::error!("Auto-updater unexpectedly terminated."),
					Err(e) => log::error!("Runner error: {}", e),
				}
				runner.terminate_proc_and_wait()?;
			}
		};
		Ok(())
	}

	async fn auto_update(runner: &mut impl RunnerExt) -> Result<(), Error> {
		// Create all directories for the `download_path` if they don't already exist.
		fs::create_dir_all(runner.download_path())?;

		let release: ClientRelease = runner
			.try_get_release()
			.await?
			.expect("No current client release set on-chain.");

		if runner.try_load_downloaded_binary(&release).is_err() {
			// something went wrong while loading the binary: it is outdated,
			// non-existent, or something else went wrong. In all of these
			// case, try to download the newest binary

			runner.download_binary(release).await?;
		}

		runner.run_binary()?;

		loop {
			runner.maybe_restart_client()?;
			match runner.try_get_release().await {
				Err(error) => {
					// Create new RPC client, assuming it's a websocket connection error.
					// We can't detect if it's a websocket error (https://github.com/paritytech/subxt/issues/1190)
					// so we just close and reopen the connection.
					// replace with https://github.com/pendulum-chain/spacewalk/issues/521 eventually
					log::error!("Error getting release: {}", error);
					log::info!("Reopening connection to RPC endpoint...");
					match runner.reopen_subxt_api().await {
						Ok(_) => log::info!("Connection to RPC endpoint reopened"),
						Err(e) => log::error!("Failed to reopen connection: {}", e),
					}
				},
				Ok(Some(new_release)) => {
					let maybe_downloaded_release = runner.downloaded_release();
					let downloaded_release =
						maybe_downloaded_release.as_ref().ok_or(Error::NoDownloadedRelease)?;
					if new_release.checksum != downloaded_release.checksum {
						log::info!("Found new client release, updating...");

						// Wait for child process to finish completely.
						// To ensure there can't be two client processes using the same resources
						// (such as the Stellar wallet for vaults).
						runner.terminate_proc_and_wait()?;

						// Delete old release
						runner.delete_downloaded_release()?;

						// Download new release
						runner.download_binary(new_release).await?;

						// Run the downloaded release
						runner.run_binary()?;
					}
				},
				_ => (),
			}
			tokio::time::sleep(BLOCK_TIME).await;
		}
	}

	fn maybe_restart_client(runner: &mut impl RunnerExt) -> Result<(), Error> {
		if !runner.check_child_proc_alive()? {
			runner.run_binary()?;
		}
		Ok(())
	}

	fn check_child_proc_alive(runner: &mut impl RunnerExt) -> Result<bool, Error> {
		if let Some(child) = runner.child_proc() {
			// `try_wait` only returns (Some) if the child has already exited,
			// without actually doing any waiting
			match child.try_wait()? {
				Some(status) => {
					log::info!("Child exited with: {status}");
					runner.set_child_proc(None);
					return Ok(false)
				},
				None => return Ok(true),
			}
		}
		Ok(false)
	}

	fn run_binary(
		runner: &mut impl RunnerExt,
		stdout_mode: impl Into<Stdio>,
	) -> Result<Child, Error> {
		if runner.child_proc().is_some() {
			return Err(Error::ChildProcessExists)
		}
		let downloaded_release =
			runner.downloaded_release().as_ref().ok_or(Error::NoDownloadedRelease)?;
		let mut command = Command::new(downloaded_release.path.as_os_str());
		command.args(runner.client_args().clone()).stdout(stdout_mode);
		let child = retry_with_log(
			|| command.spawn().map_err(Into::into),
			"Failed to spawn child process".to_string(),
		)?;
		log::info!("Client started, with pid {}", child.id());
		Ok(child)
	}
}

impl Drop for Runner {
	fn drop(&mut self) {
		if self.check_child_proc_alive().expect("Failed to check child process status") {
			if let Err(e) = self.terminate_proc_and_wait() {
				log::warn!("Failed to terminate child process: {}", e);
			}
		}
	}
}

#[async_trait]
pub trait RunnerExt {
	fn subxt_api(&self) -> &OnlineClient<PolkadotConfig>;
	/// Close the current RPC client and create a new one to the same endpoint.
	async fn reopen_subxt_api(&mut self) -> Result<(), Error>;
	fn client_args(&self) -> &Vec<String>;
	fn child_proc(&mut self) -> &mut Option<Child>;
	fn set_child_proc(&mut self, child_proc: Option<Child>);
	fn downloaded_release(&self) -> &Option<DownloadedRelease>;
	fn set_downloaded_release(&mut self, downloaded_release: Option<DownloadedRelease>);
	fn download_path(&self) -> &PathBuf;
	fn client_type(&self) -> ClientType;
	/// Read the current client release from the parachain, retrying for `RETRY_TIMEOUT` if there is
	/// a network error.
	async fn try_get_release(&self) -> Result<Option<ClientRelease>, Error>;
	/// Download the client binary and make it executable, retrying for `RETRY_TIMEOUT` if there is
	/// a network error.
	async fn download_binary(&mut self, release: ClientRelease) -> Result<(), Error>;
	/// Convert a release URI (e.g. a GitHub link) to an executable name and OS path (after
	/// download)
	fn get_bin_path(&self, uri: &str) -> Result<(String, PathBuf), Error>;
	/// Remove downloaded release from the file system. This is only supposed to occur _after_ the
	/// client process has been killed. In case of failure, removing is retried for `RETRY_TIMEOUT`.
	fn delete_downloaded_release(&mut self) -> Result<(), Error>;
	/// Spawn a the client as a child process with the CLI arguments set in the `Runner`, retrying
	/// for `RETRY_TIMEOUT`.
	fn run_binary(&mut self) -> Result<(), Error>;
	/// Send a `SIGTERM` to the child process (via the underlying `kill` system call).
	/// If `kill` returns an error code, the operation is retried for `RETRY_TIMEOUT`.
	fn terminate_proc_and_wait(&mut self) -> Result<(), Error>;
	/// Main loop, checks the parachain for new releases and updates the client accordingly.
	fn auto_update(&mut self) -> BoxFuture<'_, Result<(), Error>>;
	/// Returns whether the child is alive and sets the `runner.child` field to `None` if not.
	fn check_child_proc_alive(&mut self) -> Result<bool, Error>;
	/// If the child process crashed, start it again
	fn maybe_restart_client(&mut self) -> Result<(), Error>;
	/// If a client binary exists on disk, load it.
	fn try_load_downloaded_binary(&mut self, release: &ClientRelease) -> Result<(), Error>;

	fn checksum_matches(&self, checksum: H256, release: &ClientRelease) -> Result<(), Error>;
}

#[async_trait]
impl RunnerExt for Runner {
	fn subxt_api(&self) -> &OnlineClient<PolkadotConfig> {
		&self.subxt_api
	}

	async fn reopen_subxt_api(&mut self) -> Result<(), Error> {
		// Create new RPC client
		let new_api = try_create_subxt_api(&self.opts.parachain_ws).await?;
		// We don't need to explicitly close the old one as it should close when dropped
		self.subxt_api = new_api;
		Ok(())
	}

	fn client_args(&self) -> &Vec<String> {
		&self.opts.client_args
	}

	fn child_proc(&mut self) -> &mut Option<Child> {
		&mut self.child_proc
	}

	fn set_child_proc(&mut self, child_proc: Option<Child>) {
		self.child_proc = child_proc;
	}

	fn downloaded_release(&self) -> &Option<DownloadedRelease> {
		&self.downloaded_release
	}

	fn set_downloaded_release(&mut self, downloaded_release: Option<DownloadedRelease>) {
		self.downloaded_release = downloaded_release;
	}

	fn download_path(&self) -> &PathBuf {
		&self.opts.download_path
	}

	fn client_type(&self) -> ClientType {
		self.opts.client_type.clone()
	}

	async fn try_get_release(&self) -> Result<Option<ClientRelease>, Error> {
		Runner::try_get_release(self).await
	}

	fn run_binary(&mut self) -> Result<(), Error> {
		let child = Runner::run_binary(self, Stdio::inherit())?;
		self.child_proc = Some(child);
		Ok(())
	}

	async fn download_binary(&mut self, release: ClientRelease) -> Result<(), Error> {
		let _downloaded_release = Runner::download_binary(self, release).await?;
		Ok(())
	}

	fn get_bin_path(&self, uri: &str) -> Result<(String, PathBuf), Error> {
		Runner::get_bin_path(self, uri)
	}

	fn delete_downloaded_release(&mut self) -> Result<(), Error> {
		Runner::delete_downloaded_release(self)?;
		Ok(())
	}

	fn terminate_proc_and_wait(&mut self) -> Result<(), Error> {
		Runner::terminate_proc_and_wait(self)?;
		Ok(())
	}

	fn auto_update(&mut self) -> BoxFuture<'_, Result<(), Error>> {
		Runner::auto_update(self).into_future().boxed()
	}

	fn check_child_proc_alive(&mut self) -> Result<bool, Error> {
		Runner::check_child_proc_alive(self)
	}

	fn maybe_restart_client(&mut self) -> Result<(), Error> {
		Runner::maybe_restart_client(self)
	}

	fn try_load_downloaded_binary(&mut self, release: &ClientRelease) -> Result<(), Error> {
		Runner::try_load_downloaded_binary(self, release)
	}

	fn checksum_matches(&self, checksum: H256, release: &ClientRelease) -> Result<(), Error> {
		Runner::checksum_matches(checksum, release)
	}
}

#[async_trait]
pub trait StorageReader {
	async fn read_chain_storage<T: 'static + Decode + Debug>(
		&self,
		subxt_api: &OnlineClient<PolkadotConfig>,
	) -> Result<Option<T>, Error>;
}

#[async_trait]
impl StorageReader for Runner {
	async fn read_chain_storage<T: 'static + Decode + Debug>(
		&self,
		subxt_api: &OnlineClient<PolkadotConfig>,
	) -> Result<Option<T>, Error> {
		Runner::read_chain_storage(&self.client_type(), subxt_api).await
	}
}

pub async fn try_create_subxt_api(url: &str) -> Result<OnlineClient<PolkadotConfig>, Error> {
	retry_with_log_async(
		|| subxt_api(url).into_future().boxed(),
		"Error creating RPC client".to_string(),
	)
	.await
}

async fn subxt_api(url: &str) -> Result<OnlineClient<PolkadotConfig>, Error> {
	Ok(OnlineClient::from_url(url).await?)
}

pub fn retry_with_log<T, F>(mut f: F, log_msg: String) -> Result<T, Error>
where
	F: FnMut() -> Result<T, Error>,
{
	// We store the error to return it if the backoff is exhausted
	let mut error = None;

	// We retry for the number of retries calculated based on the `RETRY_TIMEOUT` and
	// `RETRY_INTERVAL`
	let retries = RETRY_TIMEOUT.as_secs().checked_div(RETRY_INTERVAL.as_secs()).unwrap_or(1);
	for index in 0..retries {
		match f() {
			Ok(result) => return Ok(result),
			Err(err) => {
				if index == 0 {
					log::warn!("{}: {}. Retrying...", log_msg, err.to_string());
				}

				std::thread::sleep(RETRY_INTERVAL);
				error = Some(err)
			},
		}
	}

	let error = error.expect("Error should not be None if we reach here.");
	log::warn!("{}: {}. Retries exhausted.", log_msg, error.to_string());

	Err(error)
}

pub async fn retry_with_log_async<'a, T, F, E>(f: F, log_msg: String) -> Result<T, Error>
where
	F: Fn() -> BoxFuture<'a, Result<T, E>>,
	E: Into<Error> + Sized + Display,
{
	// We store the error to return it if the backoff is exhausted
	let mut error = None;

	// We retry for the number of retries calculated based on the `RETRY_TIMEOUT` and
	// `RETRY_INTERVAL`
	let retries = RETRY_TIMEOUT.as_secs().checked_div(RETRY_INTERVAL.as_secs()).unwrap_or(1);
	for index in 0..retries {
		match f().await {
			Ok(result) => return Ok(result),
			Err(err) => {
				if index == 0 {
					log::warn!("{}: {}. Retrying...", log_msg, err.to_string());
				}

				tokio::time::sleep(RETRY_INTERVAL).await;
				error = Some(err)
			},
		}
	}

	let error = error.expect("Error should not be None if we reach here.");
	log::warn!("{}: {}. Retries exhausted.", log_msg, error.to_string());

	Err(error.into())
}

#[cfg(test)]
mod tests {
	use async_trait::async_trait;
	use codec::Decode;

	use futures::future::BoxFuture;
	use sp_core::H256;
	use tempdir::TempDir;

	use std::{
		fmt::Debug,
		fs::{self, File},
		io::Write,
		os::unix::prelude::PermissionsExt,
		path::PathBuf,
		process::{Child, Command, Stdio},
		str::FromStr,
		thread,
	};

	use signal_hook::consts::*;
	use signal_hook_tokio::Signals;

	use crate::error::Error;

	use super::*;

	use sysinfo::{Pid, System, SystemExt};

	macro_rules! assert_err {
		($result:expr, $err:pat) => {{
			match $result {
				Err($err) => (),
				Ok(v) => panic!("assertion failed: Ok({:?})", v),
				_ => panic!("expected: Err($err)"),
			}
		}};
	}

	mockall::mock! {
		Runner {}

		#[async_trait]
		pub trait RunnerExt {
			fn subxt_api(&self) -> &OnlineClient<PolkadotConfig>;
			async fn reopen_subxt_api(&mut self) -> Result<(), Error>;
			fn client_args(&self) -> &Vec<String>;
			fn child_proc(&mut self) -> &mut Option<Child>;
			fn set_child_proc(&mut self, child_proc: Option<Child>);
			fn downloaded_release(&self) -> &Option<DownloadedRelease>;
			fn set_downloaded_release(&mut self, downloaded_release: Option<DownloadedRelease>);
			fn download_path(&self) -> &PathBuf;
			fn client_type(&self) -> ClientType;
			async fn try_get_release(&self) -> Result<Option<ClientRelease>, Error>;
			async fn download_binary(&mut self, release: ClientRelease) -> Result<(), Error>;
			fn get_bin_path(&self, uri: &str) -> Result<(String, PathBuf), Error>;
			fn delete_downloaded_release(&mut self) -> Result<(), Error>;
			fn run_binary(&mut self) -> Result<(), Error>;
			fn terminate_proc_and_wait(&mut self) -> Result<(), Error>;
			fn auto_update(&mut self) ->  BoxFuture<'static, Result<(), Error>>;
			fn check_child_proc_alive(&mut self) -> Result<bool, Error>;
			fn maybe_restart_client(&mut self) -> Result<(), Error>;
			fn try_load_downloaded_binary(&mut self, release: &ClientRelease) -> Result<(), Error>;
			fn checksum_matches(&self, checksum:H256, release: &ClientRelease) -> Result<(),Error>;
		}

		#[async_trait]
		pub trait StorageReader {
			async fn read_chain_storage<T: 'static + Decode + Debug>(
				&self,
				subxt_api: &OnlineClient<PolkadotConfig>,
			) -> Result<Option<T>, Error>;
		}
	}

	// Test the backoff/retry implementation
	#[tokio::test]
	async fn test_retry_with_log() {
		let expected_retries =
			RETRY_TIMEOUT.as_secs().checked_div(RETRY_INTERVAL.as_secs()).unwrap_or(0);

		let counter = std::sync::Mutex::new(0);
		let result: Result<(), Error> = retry_with_log(
			|| {
				let mut counter = counter.lock().unwrap();
				*counter += 1;
				if (*counter) > expected_retries {
					panic!("Backoff retries more often than expected")
				}

				// We always return an error so that we retry. It can be any error
				Err(Error::ProcessTerminationFailure)
			},
			"Error. Retrying".to_string(),
		);
		// We expect to get the returned error as a result once all retries are exhausted
		assert!(result.is_err());
		let counter = *counter.lock().unwrap();
		assert_eq!(counter, expected_retries);

		let counter = std::sync::Mutex::new(0);
		let result: Result<(), Error> = retry_with_log_async(
			|| {
				let mut counter = counter.lock().unwrap();
				*counter += 1;
				if (*counter) > expected_retries {
					panic!("Backoff retries more often than expected")
				}

				// We always return an error so that we retry. It can be any error
				Box::pin(async { Err(Error::ProcessTerminationFailure) })
			},
			"Error. Retrying".to_string(),
		)
		.await;
		// We expect to get the returned error as a result once all retries are exhausted
		assert!(result.is_err());
		let counter = *counter.lock().unwrap();
		assert_eq!(counter, expected_retries);
	}

	//Before running this test, ensure uri and checksum of the test file match!
	#[tokio::test]
	async fn test_runner_download_binary() {
		let mut runner = MockRunner::default();
		let tmp = TempDir::new("runner-tests").expect("failed to create tempdir");
		let mock_path = tmp.path().to_path_buf().join("vault-rococo");
		let moved_mock_path = tmp.path().to_path_buf().join("vault-rococo");
		let mock_bin_name = "vault-rococo".to_string();

		let client_release = ClientRelease {
			uri:
				"https://github.com/pendulum-chain/spacewalk/releases/download/v1.0.0/vault-rococo"
					.to_string(),
			checksum: H256::from_str(
				&"0x82b24c352f709b10777b3a7da804dc4e15763afdcb4fb212862bdb73d6632d5e".to_string(),
			)
			.expect("checksum built from test file"),
		};

		runner
			.expect_get_bin_path()
			.returning(move |_| Ok(("vault-rococo".to_string(), moved_mock_path.clone())));

		runner.expect_downloaded_release().return_const(None);
		runner.expect_set_downloaded_release().return_const(());
		runner.expect_checksum_matches().returning(|_, _| Ok(()));

		let downloaded_release =
			Runner::download_binary(&mut runner, client_release.clone()).await.unwrap();
		assert_eq!(
			downloaded_release,
			DownloadedRelease {
				checksum: client_release.checksum,
				path: mock_path.clone(),
				bin_name: mock_bin_name
			}
		);

		let meta = std::fs::metadata(mock_path.clone()).unwrap();
		// The POSIX mode returned by `Permissions::mode()` contains two kinds of
		// information: the file type code, and the access permission bits.
		// Since the executable is a regular file, its file type code fits the
		// `S_IFREG` bit mask (`0o0100000`).
		// Sources:
		// - https://www.gnu.org/software/libc/manual/html_node/Testing-File-Type.html
		// - https://en.wikibooks.org/wiki/C_Programming/POSIX_Reference/sys/stat.h
		assert_eq!(
			meta.permissions(),
			// Expect the mode to include both the file type (`0100000`) and file permissions
			// (`700`).
			fs::Permissions::from_mode(0o0100700)
		);
	}

	#[tokio::test]
	async fn test_runner_binary_is_not_redownloaded() {
		let mut runner = MockRunner::default();
		let downloaded_release = DownloadedRelease::default();

		runner
			.expect_downloaded_release()
			.return_const(Some(downloaded_release.clone()));

		// No release should be set in the runner
		runner
			.expect_set_downloaded_release()
			.times(0)
			.returning(|_| panic!("Unexpected call"));

		runner.expect_checksum_matches().returning(|_, _| Ok(()));

		// The latest on-chain release matches the currently downloaded one
		let new_downloaded_release =
			Runner::download_binary(&mut runner, ClientRelease::default()).await.unwrap();

		assert_eq!(downloaded_release, new_downloaded_release);
	}

	#[tokio::test]
	async fn test_runner_get_bin_path() {
		let clients = [ClientType::Vault];
		let mock_path = PathBuf::from_str("./mock_download_dir").unwrap();
		for client in clients {
			let mut runner = MockRunner::default();
			runner.expect_download_path().return_const(mock_path.clone());
			runner.expect_client_type().return_const(client.clone());
			let (bin_name, bin_path) = Runner::get_bin_path(
				&runner,
				"https://downloads.pendulumchain.tech/spacewalk/vault-rococo",
			)
			.unwrap();
			assert_eq!(bin_name, "vault-rococo");
			assert_eq!(bin_path, mock_path.join(bin_name));
		}
	}

	#[tokio::test]
	async fn test_runner_delete_downloaded_release() {
		let tmp = TempDir::new("runner-tests").expect("failed to create tempdir");
		// Create dummy file
		let mock_path = tmp.path().join("mock_file");
		File::create(mock_path.clone()).unwrap();

		let mut runner = MockRunner::default();
		let downloaded_release = DownloadedRelease {
			checksum: H256::default(),
			path: mock_path.clone(),
			bin_name: String::default(),
		};
		runner.expect_downloaded_release().return_const(Some(downloaded_release));
		runner.expect_set_downloaded_release().return_const(());

		Runner::delete_downloaded_release(&mut runner).unwrap();
		assert_eq!(mock_path.exists(), false);
	}

	#[tokio::test]
	async fn test_runner_terminate_proc_and_wait() {
		// spawn long-running child process
		let mut runner = MockRunner::default();
		runner
			.expect_child_proc()
			.returning(|| Some(Command::new("sleep").arg("100").spawn().unwrap()));
		runner.expect_set_child_proc().return_const(());
		let pid = Runner::terminate_proc_and_wait(&mut runner).unwrap();
		let pid_i32: i32 = pid.try_into().unwrap();
		let s = System::new();
		// Get all running processes
		let processes = s.processes();
		// Get the child process based on its pid
		let child_process = processes.get(&Pid::from(pid_i32));

		assert_eq!(child_process.is_none(), true);
	}

	#[tokio::test]
	async fn test_runner_run_binary_with_retry() {
		let tmp = TempDir::new("runner-tests").expect("failed to create tempdir");

		let mock_executable_path = tmp.path().join("print_cli_input");
		{
			let mut file = OpenOptions::new()
				.read(true)
				.write(true)
				.mode(0o700)
				.create(true)
				.open(mock_executable_path.clone())
				.unwrap();

			// Script that prints CLI input to stdout
			file.write_all(b"#!/bin/bash\necho $@").unwrap();

			file.sync_all().unwrap();
			// drop `file` here to close it and avoid `ExecutableFileBusy` errors
		}

		let mut runner = MockRunner::default();
		let mock_vault_args: Vec<String> = vec![
			"--keyring",
			"alice",
			"--stellar-vault-secret-key-filepath",
			"secret",
			"--stellar-overlay-config-filepath",
			"config.json",
			"--keyfile",
		]
		.iter()
		.map(|s| s.to_string())
		.collect();

		let mock_downloaded_release = DownloadedRelease {
			checksum: H256::default(),
			path: mock_executable_path.clone(),
			bin_name: String::default(),
		};
		runner.expect_child_proc().return_var(None);
		runner.expect_downloaded_release().return_const(Some(mock_downloaded_release));
		runner.expect_client_args().return_const(mock_vault_args.clone());
		runner.expect_set_child_proc().return_const(());
		let child = Runner::run_binary(&mut runner, Stdio::piped()).unwrap();

		let output = child.wait_with_output().unwrap();

		let mut expected_output = mock_vault_args.join(" ");
		expected_output.push('\n');
		assert_eq!(output.stdout, expected_output.as_bytes());
	}

	#[tokio::test]
	async fn test_runner_terminate_child_proc_on_signal() {
		let mut runner = MockRunner::default();
		runner.expect_terminate_proc_and_wait().once().returning(|| Ok(()));
		runner.expect_auto_update().returning(|| {
			Box::pin(async {
				tokio::time::sleep(Duration::from_millis(100_000)).await;
				Ok(())
			})
		});
		let shutdown_signals = Signals::new(&[SIGHUP, SIGTERM, SIGINT, SIGQUIT]).unwrap();
		let task = tokio::spawn(Runner::run(Box::new(runner), shutdown_signals));
		// Wait for the signals iterator to be polled
		// This `sleep` is based on the test case in `signal-hook-tokio` itself:
		// https://github.com/vorner/signal-hook/blob/a9e5ca5e46c9c8e6de89ff1b3ce63c5ff89cd708/signal-hook-tokio/tests/tests.rs#L50
		thread::sleep(Duration::from_millis(100));
		signal_hook::low_level::raise(SIGTERM).unwrap();
		task.await.unwrap().unwrap();
	}

	#[tokio::test]
	async fn test_runner_terminate_child_proc_on_crash() {
		let mut runner = MockRunner::default();
		// Assume the auto-updater crashes
		runner.expect_auto_update().returning(|| {
			// return an arbitrary error
			Box::pin(async { Err(Error::ProcessTerminationFailure) })
		});
		// The child process must be killed before shutting down the runner
		runner.expect_terminate_proc_and_wait().once().returning(|| Ok(()));

		let shutdown_signals = Signals::new(&[]).unwrap();
		let task = tokio::spawn(Runner::run(Box::new(runner), shutdown_signals));
		task.await.unwrap().unwrap();
	}

	#[tokio::test]
	async fn test_runner_child_restarts_if_crashed() {
		let mut runner = MockRunner::default();
		runner.expect_check_child_proc_alive().returning(|| Ok(false));

		// The test passes as long as `run_binary` is called
		runner.expect_run_binary().once().returning(|| Ok(()));
		Runner::maybe_restart_client(&mut runner).unwrap();
	}

	#[tokio::test]
	async fn test_runner_terminate_child_process_does_not_throw() {
		let mut runner = MockRunner::default();
		runner.expect_child_proc().return_var(None);
		assert_eq!(Runner::terminate_proc_and_wait(&mut runner).unwrap(), 0);
	}

	#[tokio::test]
	async fn test_runner_loading_binary_fails() {
		let mut runner = MockRunner::default();

		runner.expect_get_bin_path().returning(move |_| {
			Ok((
				"client".to_string(),
				TempDir::new("runner-tests").unwrap().into_path().join("client"),
			))
		});

		assert_err!(
			Runner::try_load_downloaded_binary(&mut runner, &Default::default()),
			Error::NoDownloadedRelease
		);
	}

	#[tokio::test]
	async fn test_runner_loading_existing_binary_works() {
		let tmp = TempDir::new("runner-tests").expect("failed to create tempdir");
		let mock_path = tmp.path().to_path_buf().join("client");
		let moved_mock_path = tmp.path().to_path_buf().join("client");
		let mut runner = MockRunner::default();

		{
			let mut file = OpenOptions::new()
				.read(true)
				.write(true)
				.create(true)
				.open(mock_path.clone())
				.unwrap();
			file.write_all(b"dummy file").unwrap();
			file.sync_all().unwrap();
		}

		runner
			.expect_get_bin_path()
			.returning(move |_| Ok(("client".to_string(), moved_mock_path.clone())));
		runner.expect_set_downloaded_release().returning(|release_option| {
			let checksum = release_option.unwrap().checksum;
			let expected_checksum = H256::from_slice(&[
				81, 216, 24, 152, 19, 116, 164, 71, 240, 135, 102, 16, 253, 43, 174, 235, 145, 29,
				213, 173, 96, 198, 230, 180, 210, 182, 182, 121, 139, 165, 192, 113,
			]);
			assert_eq!(checksum, expected_checksum);
		});
		runner.expect_checksum_matches().returning(|_, _| Ok(()));

		let release = ClientRelease {
			checksum: H256::from_slice(&[
				81, 216, 24, 152, 19, 116, 164, 71, 240, 135, 102, 16, 253, 43, 174, 235, 145, 29,
				213, 173, 96, 198, 230, 180, 210, 182, 182, 121, 139, 165, 192, 113,
			]),
			..Default::default()
		};
		Runner::try_load_downloaded_binary(&mut runner, &release).unwrap();
	}

	#[tokio::test]
	async fn test_try_load_downloaded_binary_checks_checksum() {
		let tmp = TempDir::new("runner-tests").expect("failed to create tempdir");
		let mock_path = tmp.path().to_path_buf().join("client");
		let moved_mock_path = tmp.path().to_path_buf().join("client");
		let mut runner = MockRunner::default();

		{
			let mut file = OpenOptions::new()
				.read(true)
				.write(true)
				.create(true)
				.open(mock_path.clone())
				.unwrap();
			file.write_all(b"dummy file").unwrap();
			file.sync_all().unwrap();
		}

		runner
			.expect_get_bin_path()
			.returning(move |_| Ok(("client".to_string(), moved_mock_path.clone())));

		runner.expect_checksum_matches().returning(|_, _| Err(Error::IncorrectChecksum));

		assert_err!(
			Runner::try_load_downloaded_binary(&mut runner, &Default::default()),
			Error::IncorrectChecksum
		);
	}

	#[tokio::test]
	async fn test_runner_invalid_binary_prompts_download() {
		let tmp = TempDir::new("runner-tests").expect("failed to create tempdir");
		let mock_path = tmp.path().to_path_buf().join("client");
		let mut runner = MockRunner::default();

		runner.expect_download_path().return_const(mock_path.clone());
		runner
			.expect_try_load_downloaded_binary()
			.returning(|_| Err(Error::IncorrectChecksum));

		runner
			.expect_try_get_release()
			.once()
			.returning(|| Ok(Some(ClientRelease::default())));
		runner.expect_download_binary().once().returning(|release| {
			assert_eq!(release, ClientRelease::default());
			Ok(())
		});

		// return arbitrary error to terminate the `auto_update` function
		runner
			.expect_run_binary()
			.once()
			.returning(|| Err(Error::ProcessTerminationFailure));

		assert_err!(Runner::auto_update(&mut runner).await, Error::ProcessTerminationFailure);
	}

	#[tokio::test]
	async fn test_runner_nonexistent_binary_prompts_download() {
		let tmp = TempDir::new("runner-tests").expect("failed to create tempdir");
		let mock_path = tmp.path().to_path_buf().join("client");
		let mut runner = MockRunner::default();

		runner.expect_download_path().return_const(mock_path.clone());
		runner
			.expect_try_load_downloaded_binary()
			.returning(|_| Err(Error::NoDownloadedRelease));

		runner
			.expect_try_get_release()
			.once()
			.returning(|| Ok(Some(ClientRelease::default())));
		runner.expect_download_binary().once().returning(|release| {
			assert_eq!(release, ClientRelease::default());
			Ok(())
		});

		// return arbitrary error to terminate the `auto_update` function
		runner
			.expect_run_binary()
			.once()
			.returning(|| Err(Error::ProcessTerminationFailure));

		assert_err!(Runner::auto_update(&mut runner).await, Error::ProcessTerminationFailure);
	}

	#[tokio::test]
	async fn test_runner_different_checksum_prompts_download() {
		let tmp = TempDir::new("runner-tests").expect("failed to create tempdir");
		let mock_path = tmp.path().to_path_buf().join("client");
		let mut runner = MockRunner::default();

		runner.expect_download_path().return_const(mock_path.clone());
		runner.expect_try_load_downloaded_binary().returning(|_| Ok(()));
		runner.expect_run_binary().once().returning(|| Ok(()));
		runner.expect_maybe_restart_client().once().returning(|| Ok(()));

		runner.expect_try_get_release().times(2).returning(|| {
			Ok(Some(ClientRelease { uri: Default::default(), checksum: H256::from_low_u64_be(10) }))
		});
		runner.expect_downloaded_release().once().return_const(Some(DownloadedRelease {
			path: Default::default(),
			bin_name: Default::default(),
			checksum: H256::from_low_u64_be(11),
		}));

		// return arbitrary error to terminate the `auto_update` function
		runner
			.expect_terminate_proc_and_wait()
			.returning(|| Err(Error::ProcessTerminationFailure));

		assert_err!(Runner::auto_update(&mut runner).await, Error::ProcessTerminationFailure);
	}
}
