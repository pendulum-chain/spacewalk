use std::{
	fmt::{Debug, Formatter},
	marker::{Send, Sync},
};

use async_trait::async_trait;
use futures::{channel::mpsc::Receiver, *};

use runtime::{
	AccountId, BlockNumber, Error as RuntimeError, IssuePallet, IssueRequestStatus, ReplacePallet,
	ReplaceRequestStatus, SecurityPallet, UtilFuncs, H256,
};

use super::Error;

pub enum Event {
	/// new issue requested / replace accepted
	Opened,
	/// issue / replace successfully executed
	Executed(H256),
	/// new *active* parachain block
	ParachainBlock(BlockNumber),
}

pub struct CancellationScheduler<P: IssuePallet + ReplacePallet + UtilFuncs + Clone> {
	parachain_rpc: P,
	vault_id: AccountId,
	parachain_height: BlockNumber,
}

#[derive(Copy, Clone, Debug, PartialEq)]
struct ActiveRequest {
	id: H256,
	parachain_deadline_height: u32,
}

pub struct UnconvertedOpenTime {
	id: H256,
	parachain_open_height: u32,
	period: u32,
}

impl Debug for UnconvertedOpenTime {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		let id = hex::encode(self.id);
		return write!(
			f,
			"UnconvertedOpenTime:{{ id: {}, parachain_open_height: {}, period: {} }}",
			id, self.parachain_open_height, self.period
		)
	}
}

#[derive(PartialEq, Debug)]
enum ListState {
	Valid,
	Invalid,
}

/// Trait to abstract over issue & replace cancellation
#[async_trait]
pub trait Canceller<P> {
	/// either "replace" or "issue"; used for logging
	const TYPE_NAME: &'static str;

	/// Gets a list of open replace/issue requests
	async fn get_open_requests(
		parachain_rpc: &P,
		vault_id: AccountId,
	) -> Result<Vec<UnconvertedOpenTime>, Error>
	where
		P: 'async_trait;

	/// Gets the timeout period in number of blocks
	async fn get_period(parachain_rpc: &P) -> Result<u32, Error>
	where
		P: 'async_trait;

	/// Cancels the issue/replace
	async fn cancel_request(parachain_rpc: &P, request_id: H256) -> Result<(), Error>
	where
		P: 'async_trait;
}

pub struct IssueCanceller;

#[async_trait]
impl<P: IssuePallet + ReplacePallet + Clone + Send + Sync> Canceller<P> for IssueCanceller {
	const TYPE_NAME: &'static str = "issue";

	async fn get_open_requests(
		parachain_rpc: &P,
		vault_id: AccountId,
	) -> Result<Vec<UnconvertedOpenTime>, Error>
	where
		P: 'async_trait,
	{
		let ret = parachain_rpc
			.get_vault_issue_requests(vault_id)
			.await?
			.iter()
			.filter(|(_, issue)| issue.status == IssueRequestStatus::Pending)
			.map(|(id, issue)| UnconvertedOpenTime {
				id: *id,
				parachain_open_height: issue.opentime,
				period: issue.period,
			})
			.collect();
		Ok(ret)
	}

	async fn get_period(parachain_rpc: &P) -> Result<u32, Error>
	where
		P: 'async_trait,
	{
		Ok(parachain_rpc.get_issue_period().await?)
	}

	async fn cancel_request(parachain_rpc: &P, request_id: H256) -> Result<(), Error>
	where
		P: 'async_trait,
	{
		Ok(parachain_rpc.cancel_issue(request_id).await?)
	}
}

pub struct ReplaceCanceller;

#[async_trait]
impl<P: IssuePallet + ReplacePallet + Send + Sync> Canceller<P> for ReplaceCanceller {
	const TYPE_NAME: &'static str = "replace";

	async fn get_open_requests(
		parachain_rpc: &P,
		vault_id: AccountId,
	) -> Result<Vec<UnconvertedOpenTime>, Error>
	where
		P: 'async_trait,
	{
		let ret = parachain_rpc
			.get_new_vault_replace_requests(vault_id)
			.await?
			.iter()
			.filter(|(_, replace)| replace.status == ReplaceRequestStatus::Pending)
			.map(|(id, replace)| UnconvertedOpenTime {
				id: *id,
				parachain_open_height: replace.accept_time,
				period: replace.period,
			})
			.collect();
		Ok(ret)
	}

	async fn get_period(parachain_rpc: &P) -> Result<u32, Error>
	where
		P: 'async_trait,
	{
		Ok(parachain_rpc.get_replace_period().await?)
	}

	async fn cancel_request(parachain_rpc: &P, request_id: H256) -> Result<(), Error>
	where
		P: 'async_trait,
	{
		Ok(parachain_rpc.cancel_replace(request_id).await?)
	}
}

// verbose drain_filter
fn drain_expired(requests: &mut Vec<ActiveRequest>, current_height: u32) -> Vec<ActiveRequest> {
	let mut expired = Vec::new();
	let has_expired = |request: &ActiveRequest| current_height > request.parachain_deadline_height;
	let mut i = 0;
	while i != requests.len() {
		if has_expired(&requests[i]) {
			let req = requests.remove(i);
			expired.push(req);
		} else {
			i += 1;
		}
	}
	expired
}

/// The actual cancellation scheduling and handling
impl<P: IssuePallet + ReplacePallet + UtilFuncs + SecurityPallet + Clone> CancellationScheduler<P> {
	pub fn new(
		parachain_rpc: P,
		parachain_height: BlockNumber,
		vault_id: AccountId,
	) -> CancellationScheduler<P> {
		CancellationScheduler { parachain_rpc, vault_id, parachain_height }
	}

	/// Listens for issuing events (i.e. issue received/executed). When
	/// the issue period has expired without the issue having been executed,
	/// this function will attempt to call cancel_event to get the collateral back.
	/// On start, queries open issues and schedules cancellation for these as well.
	///
	/// # Arguments
	///
	/// *`event_listener`: channel that signals relevant events _for this vault_.
	pub async fn handle_cancellation<T: Canceller<P>>(
		mut self,
		mut event_listener: Receiver<Event>,
	) -> Result<(), RuntimeError> {
		let mut list_state = ListState::Invalid;
		let mut active_requests: Vec<ActiveRequest> = vec![];

		loop {
			let event = event_listener.next().await.ok_or(RuntimeError::ChannelClosed)?;

			list_state = self.process_event::<T>(event, &mut active_requests, list_state).await?;
		}
	}

	async fn cancel_requests<T: Canceller<P>>(
		&self,
		active_requests: &mut Vec<ActiveRequest>,
	) -> ListState {
		let cancellable_requests = drain_expired(active_requests, self.parachain_height);

		for request in cancellable_requests {
			match T::cancel_request(&self.parachain_rpc, request.id).await {
				Ok(_) => {
					tracing::info!(
						"Successfully cancelled {} request with id #{:?}.",
						T::TYPE_NAME,
						request.id
					);
					tracing::debug!(
						"Info of the cancelled {} request: {:#?}",
						T::TYPE_NAME,
						request
					);
				},
				Err(e) => {
					// failed to cancel; get up-to-date request list in next iteration
					tracing::error!(
						"Cancellation of {} request with id #{:?} failed with error {e:?}",
						T::TYPE_NAME,
						request.id
					);
					return ListState::Invalid
				},
			}
		}
		ListState::Valid
	}

	/// Handles one timeout or event_listener event. This method is split from handle_cancellation
	/// for testing purposes
	async fn process_event<T: Canceller<P>>(
		&mut self,
		event: Event,
		active_requests: &mut Vec<ActiveRequest>,
		list_state: ListState,
	) -> Result<ListState, RuntimeError> {
		// try to get an up-to-date list of requests if we don't have it yet
		if let ListState::Invalid = list_state {
			match self.get_open_requests::<T>().await {
				Ok(x) => {
					*active_requests = x;
				},
				Err(e) => {
					active_requests.clear();
					tracing::error!("Failed to query open {} request: {e:?}", T::TYPE_NAME);
				},
			}
		}

		match event {
			Event::ParachainBlock(height) => {
				tracing::trace!(
					"Received parachain block at active height {} in process_events for {} requests",
					T::TYPE_NAME,
					height
				);
				self.parachain_height = height;
				Ok(self.cancel_requests::<T>(active_requests).await)
			},
			Event::Executed(id) => {
				tracing::debug!("Received execution event for {} request #{}", T::TYPE_NAME, id);
				active_requests.retain(|x| x.id != id);
				Ok(ListState::Valid)
			},
			Event::Opened => {
				tracing::debug!("Received 'opened' event for {} request.", T::TYPE_NAME);
				Ok(ListState::Invalid)
			},
		}
	}

	/// Gets a list of requests that have been requested from this vault
	async fn get_open_requests<T: Canceller<P>>(&mut self) -> Result<Vec<ActiveRequest>, Error> {
		let open_requests =
			T::get_open_requests(&self.parachain_rpc, self.vault_id.clone()).await?;

		tracing::trace!("List of open {} requests: {open_requests:?}", T::TYPE_NAME);

		if open_requests.is_empty() {
			return Ok(vec![])
		}

		// get current block height and request period
		let global_period = T::get_period(&self.parachain_rpc).await?;

		let ret = open_requests
			.iter()
			.map(|UnconvertedOpenTime { id, parachain_open_height, period: local_period }| {
				let period = global_period.max(*local_period);

				let parachain_deadline_height =
					parachain_open_height.checked_add(period).ok_or(Error::ArithmeticOverflow)?;

				Ok(ActiveRequest { id: *id, parachain_deadline_height })
			})
			.collect::<Result<Vec<ActiveRequest>, Error>>()?;

		Ok(ret)
	}
}
