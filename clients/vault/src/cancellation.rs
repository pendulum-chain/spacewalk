use super::Error;
use async_trait::async_trait;
use futures::{channel::mpsc::Receiver, *};
use runtime::{
    AccountId, BlockNumber, Error as RuntimeError, IssuePallet, IssueRequestStatus, ReplacePallet,
    ReplaceRequestStatus, SecurityPallet, UtilFuncs,
};
use std::marker::{Send, Sync};

use runtime::H256;

pub enum Event {
    /// new issue requested / replace accepted
    Opened,
    /// issue / replace successfully executed
    Executed(H256),
    /// new *active* parachain block
    ParachainBlock(BlockNumber),
    /// new bitcoin block included in relay
    BitcoinBlock(u32),
}

pub struct CancellationScheduler<P: IssuePallet + ReplacePallet + UtilFuncs + Clone> {
    parachain_rpc: P,
    vault_id: AccountId,
    period: Option<u32>,
    parachain_height: BlockNumber,
    bitcoin_height: u32,
}

#[derive(Copy, Clone, Debug, PartialEq)]
struct ActiveRequest {
    id: H256,
    parachain_deadline_height: u32,
    bitcoin_deadline_height: u32,
}

pub struct UnconvertedOpenTime {
    id: H256,
    parachain_open_height: u32,
    bitcoin_open_height: u32,
}

#[derive(PartialEq, Debug)]
enum ListState {
    Valid,
    Invalid,
}
