use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use tokio::sync::{mpsc, oneshot};
use tonic::{transport::Channel, Response, Status};

use super::raft_rpcs::{
    raft_rpc_client::RaftRpcClient, AppendEntriesRequest, AppendEntriesResponse, GetRequest,
    GetResponse, LogEntry, PutRequest, PutResponse, VoteRequest, VoteResponse,
};
mod funcs;

#[derive(Debug)]
pub enum Event {
    RequestVote {
        req: VoteRequest,
        tx: oneshot::Sender<Result<Response<VoteResponse>, Status>>,
    },
    AppendEntries {
        req: AppendEntriesRequest,
        tx: oneshot::Sender<Result<Response<AppendEntriesResponse>, Status>>,
    },
    ClientPutRequest {
        req: PutRequest,
        tx: oneshot::Sender<Result<Response<PutResponse>, Status>>,
    },
    ClientGetRequest {
        req: GetRequest,
        tx: oneshot::Sender<Result<Response<GetResponse>, Status>>,
    },
}

#[allow(dead_code)]
#[derive(Debug, Default, PartialEq)]
enum State {
    #[default]
    Follower,
    Candidate,
    Leader,
}

#[derive(Debug)]
pub struct ServerConfig {
    /// Configurable settings for server
    pub timeout: Duration,
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct Node {
    // volatile state
    id: String,
    state: State,
    commit_index: u64,
    last_applied: u64,
    peers: Vec<String>,
    state_machine: HashMap<String, i64>,

    // persistent state
    current_term: u64,
    voted_for: Option<String>,
    voted: bool,
    log: Vec<LogEntry>,
    next_timeout: Option<Instant>,
    config: ServerConfig,

    // volatile leader state
    next_index: HashMap<String, u64>,
    match_index: HashMap<String, u64>,

    // to receive messages from client or RPC server
    mailbox: mpsc::UnboundedReceiver<Event>,

    //connections to peers
    connections: HashMap<String, RaftRpcClient<Channel>>,
}
