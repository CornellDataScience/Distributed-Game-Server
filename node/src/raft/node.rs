use super::raft::raft_client::RaftClient;
use crate::raft::raft::{
    AppendEntriesRequest, AppendEntriesResponse, Command, GetRequest, GetResponse, LogEntry,
    PutRequest, PutResponse, VoteRequest, VoteResponse,
};
use core::panic;
use futures::executor::block_on;
use futures::future::select_all;
use std::cmp;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, oneshot};
use tonic::transport::Channel;
use tonic::{Request, Response, Status};

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
    log: Vec<LogEntry>,
    next_timeout: Option<Instant>,
    config: ServerConfig,

    // volatile leader state
    next_index: HashMap<String, u64>,
    match_index: HashMap<String, u64>,

    // to receive messages from client or RPC server
    mailbox: mpsc::UnboundedReceiver<Event>,

    //connections to peers
    connections: HashMap<String, RaftClient<Channel>>,
}

#[allow(dead_code)]
impl Node {
    pub fn new(id: String, peers: Vec<String>, mailbox: mpsc::UnboundedReceiver<Event>) -> Self {
        Self {
            id: id,
            state: State::Follower,
            commit_index: 0,
            last_applied: 0,
            peers: peers,
            state_machine: HashMap::new(),
            next_index: HashMap::new(),
            match_index: HashMap::new(),
            current_term: 1,
            voted_for: None,
            next_timeout: None,
            config: ServerConfig {
                timeout: Duration::new(1, 0),
            },
            log: Vec::new(),
            mailbox: mailbox,
            connections: HashMap::new(),
        }
    }

    fn replicate(&mut self, command: Command) -> Result<Response<PutResponse>, Status> {
        self.log.push(LogEntry {
            command: Some(command),
            term: self.current_term,
        });
        println!("{} is replicating command", self.id);
        // Once a leader has been elected, it begins servicing
        // client requests. Each client request contains a command to
        // be executed by the replicated state machines. The leader
        // appends the command to its log as a new entry, then issues
        // AppendEntries RPCs in parallel to each of the other
        // servers to replicate the entry. When the entry has been
        // safely replicated (as described below), the leader applies
        // the entry to its state machine and returns the result of that
        // execution to the client.

        // A log entry is committed once the leader
        // that created the entry has replicated it on a majority of
        // the servers (e.g., entry 7 in Figure 6)
        Err(Status::unimplemented("not implemented"))
    }

    async fn handle_get_request(
        &mut self,
        req: GetRequest,
        tx: oneshot::Sender<Result<Response<GetResponse>, Status>>,
    ) {
        if self.state != State::Leader {
            // if node is not the leader, respond with success = false and
            // indicate which leader the client should send the request to.
            return tx
                .send(Ok(Response::new(GetResponse {
                    value: 0,
                    success: false,
                    leader_id: self.voted_for.clone(),
                })))
                .unwrap_or_else(|e| println!("{:?}", e));
        }
        // exchange heartbeats with majority of cluster
        let (prev_log_index, prev_log_term) = match self.next_index.get("a") {
            None => (None, 0),
            Some(a) => (Some(*a), self.log[*a as usize].term),
        };
        let mut responses = Vec::new();
        for peer in &self.peers {
            let mut client = RaftClient::connect(peer.clone()).await.unwrap();
            let request = Request::new(AppendEntriesRequest {
                term: self.current_term,
                leader_id: self.id.clone(),
                entries: Vec::new(),
                leader_commit: self.commit_index,
                prev_log_index: prev_log_index,
                prev_log_term: prev_log_term,
            });
            responses.push(tokio::spawn(
                async move { client.append_entries(request).await },
            ));
        }
        let mut num_successes = 0;
        while !responses.is_empty() {
            match select_all(responses).await {
                (Ok(Ok(res)), _, remaining) => {
                    let r = res.into_inner();
                    if r.success {
                        num_successes += 1;
                        if num_successes >= self.peers.len() / 2 + 1 {
                            return tx
                                .send(Ok(Response::new(GetResponse {
                                    value: *self.state_machine.get(&req.key).unwrap_or(&0),
                                    success: true,
                                    leader_id: Some(self.id.clone()),
                                })))
                                .unwrap_or_else(|e| println!("{:?}", e));
                        }
                    } else if r.term > self.current_term {
                        self.current_term = r.term;
                        self.state = State::Follower;
                        self.voted_for = None;
                        return tx
                            .send(Ok(Response::new(GetResponse {
                                value: 0,
                                success: false,
                                leader_id: Some(self.id.clone()),
                            })))
                            .unwrap_or_else(|e| println!("{:?}", e));
                    }
                    responses = remaining
                }
                (_, _, remaining) => responses = remaining,
            }
        }
        return tx
            .send(Ok(Response::new(GetResponse {
                value: 0,
                success: false,
                leader_id: Some(self.id.clone()),
            })))
            .unwrap_or_else(|e| println!("{:?}", e));
    }

    async fn handle_put_request(
        &mut self,
        req: PutRequest,
        tx: oneshot::Sender<Result<Response<PutResponse>, Status>>,
    ) {
        if self.state != State::Leader {
            tx.send(Ok(Response::new(PutResponse {
                success: false,
                leader_id: self.voted_for.clone(),
            })))
            .unwrap_or_else(|e| println!("{:?}", e));
            return;
        }
        let command = Command {
            key: req.key,
            value: req.value,
        };
        tx.send(self.replicate(command))
            .unwrap_or_else(|e| println!("{:?}", e))
    }

    async fn handle_event(&mut self, event: Event) {
        match event {
            Event::RequestVote { req, tx } => tx
                .send(self.request_vote(tonic::Request::new(req)))
                .unwrap_or_else(|e| println!("{:?}", e)),
            Event::AppendEntries { req, tx } => tx
                .send(self.append_entries(tonic::Request::new(req)))
                .unwrap_or_else(|e| println!("{:?}", e)),
            Event::ClientGetRequest { req, tx } => return self.handle_get_request(req, tx).await,
            Event::ClientPutRequest { req, tx } => return self.handle_put_request(req, tx).await,
        }
    }

    pub fn refresh_timeout(self: &mut Self) {
        self.next_timeout = Some(Instant::now() + self.config.timeout);
    }

    pub fn timed_out(self: &mut Self) -> bool {
        match self.next_timeout {
            Some(t) => Instant::now() > t,
            None => false,
        }
    }

    async fn start_follower(&mut self) {
        // TODO: implement follower loop: if timer runs out,
        // change state to candidate and return from function.
        println!("starting follower");
        self.next_timeout = Some(Instant::now());
        loop {
            if self.timed_out() {
                self.state = State::Candidate;
                break;
            }
            tokio::select! {
                Some(event) = self.mailbox.recv() => {
                    self.handle_event(event).await
                }
                else => { () }
            }
        }
    }

    /// [start_candidate] starts a new node with candidate and sends RequestVoteRPCs to all peer nodes. Precondition: self.state == candidate
    async fn start_candidate(&mut self) {
        loop {
            println!("starting candidate");
            if self.state != State::Candidate {
                return;
            }
            // Increment term
            self.current_term += 1;
            // Send RequestVote to all other servers
            let mut responses = Vec::new();
            for peer in &self.peers {
                let mut client = RaftClient::connect(peer.clone()).await.unwrap();
                let (last_log_idx, last_term) = match self.log.len() {
                    0 => (None, 0),
                    i => (Some(i as u64), self.log[i - 1].term),
                };
                let mut request = Request::new(VoteRequest {
                    term: self.current_term,
                    candidate_id: self.id.clone(),
                    last_log_index: last_log_idx,
                    last_log_term: last_term,
                });
                request.set_timeout(Duration::from_secs(5));
                responses.push(tokio::spawn(
                    async move { client.request_vote(request).await },
                ));
            }

            // Check if the majority of the votes are received, change state
            let mut num_votes = 1;
            while !responses.is_empty() {
                match select_all(responses).await {
                    (Ok(Ok(res)), _, remaining) => {
                        let r = res.into_inner();
                        if r.vote_granted {
                            num_votes += 1;
                            if num_votes > (self.peers.len() + 1) / 2 {
                                self.state = State::Leader;
                                return;
                            }
                        } else if r.term > self.current_term {
                            self.current_term = r.term;
                            self.state = State::Follower;
                            self.voted_for = None;
                            return;
                        }
                        responses = remaining
                    }
                    (_, _, remaining) => responses = remaining,
                }
            }
            println!("exiting requestvote loop");
            loop {
                println!("event loop");
                match self.mailbox.try_recv() {
                    Ok(event) => {
                        println!("{:?}", event);
                        self.handle_event(event).await
                    }
                    _ => break,
                }
            }
        }
    }

    async fn start_leader(&mut self) {
        println!("starting leader in term {}", self.current_term);
        loop {
            tokio::select! {
                Some(event) = self.mailbox.recv() => {
                    self.handle_event(event).await
                }
            }
        }
    }

    pub async fn start(&mut self) {
        // Sets up connection to all peer nodes (panic if it can't find the connection - fix later :D)
        let mut connections = HashMap::new();
        self.peers.clone().into_iter().for_each(|ip| {
            connections.insert(ip.clone(), block_on(RaftClient::connect(ip)).unwrap());
        });
        self.connections = connections;
        loop {
            match self.state {
                State::Follower => self.start_follower().await,
                State::Candidate => self.start_candidate().await,
                State::Leader => self.start_leader().await,
            }
        }
    }

    fn log_newer_than(&self, log_term: u64, log_index: Option<u64>) -> bool {
        match log_index {
            None => self.log.len() > 0,
            Some(i) => {
                let last_index = self.log.len() - 1;
                let last_term = self.log[last_index].term;
                return last_term > log_term || (last_term == log_term && last_index as u64 >= i);
            }
        }
    }

    fn respond_to_vote(&self, vote_granted: bool) -> Result<Response<VoteResponse>, Status> {
        Ok(Response::new(VoteResponse {
            term: self.current_term,
            vote_granted: vote_granted,
        }))
    }

    fn respond_to_ae(&self, success: bool) -> Result<Response<AppendEntriesResponse>, Status> {
        Ok(Response::new(AppendEntriesResponse {
            term: self.current_term,
            success: success,
        }))
    }

    pub fn request_vote(
        &mut self,
        request: Request<VoteRequest>,
    ) -> Result<Response<VoteResponse>, Status> {
        let req = request.into_inner();
        println!(
            "handling request vote: {:?}, term = {:?}",
            req, self.current_term
        );
        if req.term < self.current_term {
            return self.respond_to_vote(false);
        }
        if req.term > self.current_term {
            self.state = State::Follower;
            self.current_term = req.term;
            self.voted_for = None;
        }
        let can_vote = match &self.voted_for {
            None => true,
            Some(id) => *id == req.candidate_id,
        };
        if !can_vote || self.log_newer_than(req.last_log_term, req.last_log_index) {
            return self.respond_to_vote(false);
        }
        self.voted_for = Some(req.candidate_id);
        return self.respond_to_vote(true);
    }

    // Function for when a server receives an append entries request
    pub fn append_entries(
        &mut self,
        request: Request<AppendEntriesRequest>,
    ) -> Result<Response<AppendEntriesResponse>, Status> {
        let req = request.into_inner();
        if req.term < self.current_term {
            return self.respond_to_ae(false);
        }
        self.refresh_timeout();
        if req.term >= self.current_term {
            self.state = State::Follower;
            self.current_term = req.term;
            self.voted_for = Some(req.leader_id);
        }
        if req.entries.len() == 0 {
            // heartbeat: just respond immediately
            return self.respond_to_ae(true);
        }
        let (next_i, is_consistent) = match req.prev_log_index {
            None => (0, true),
            Some(i) => {
                let i = i as usize;
                (
                    i + 1,
                    i < self.log.len() && self.log[i].term == req.prev_log_term,
                )
            }
        };
        if !is_consistent {
            return self.respond_to_ae(false);
        }
        let n = req.entries.len();
        for j in 0..n {
            if next_i + j < self.log.len() {
                self.log[next_i + j] = req.entries[j].clone();
                continue;
            }
            self.log.push(req.entries[j].clone());
        }

        if req.leader_commit > self.commit_index {
            // not sure if this is correct
            self.commit_index = cmp::min(req.leader_commit, (self.log.len() - 1) as u64);
        }

        return self.respond_to_ae(true);
    }
}

#[cfg(test)]
mod tests {
    use std::thread::sleep;

    use super::*;

    fn new_node() -> Node {
        let (_, rx) = mpsc::unbounded_channel();
        return Node::new(String::from("abc"), vec![], rx);
    }

    fn test_request_vote(receiver: &mut Node, req: VoteRequest, expected: VoteResponse) {
        match receiver.request_vote(tonic::Request::new(req)) {
            Err(e) => println!("unexpected request_vote error: {}", e),
            Ok(res) => {
                assert_eq!(res.into_inner(), expected);
            }
        }
    }

    #[test]
    fn test_vote_granted() {
        let mut follower = new_node();
        let req = VoteRequest {
            candidate_id: String::from("candidate"),
            term: 1,
            last_log_index: None,
            last_log_term: 0,
        };
        let expected_response = VoteResponse {
            term: 1,
            vote_granted: true,
        };
        test_request_vote(&mut follower, req, expected_response);
        assert_eq!(follower.voted_for, Some(String::from("candidate")));
    }
    #[test]
    fn test_candidate_older_term() {
        let mut follower = new_node();
        let req = VoteRequest {
            candidate_id: String::from("candidate"),
            term: 0,
            last_log_index: None,
            last_log_term: 0,
        };
        let expected_response = VoteResponse {
            term: 1,
            vote_granted: false,
        };
        test_request_vote(&mut follower, req, expected_response);
    }

    #[test]
    fn test_candidate_log_old_same_term() {
        let mut follower = new_node();
        follower.log.push(LogEntry {
            command: None,
            term: 1,
        });
        let req = VoteRequest {
            candidate_id: String::from("candidate"),
            term: 1,
            last_log_index: None,
            last_log_term: 0,
        };
        let expected_response = VoteResponse {
            term: 1,
            vote_granted: false,
        };
        test_request_vote(&mut follower, req, expected_response);
    }

    #[test]
    fn test_candidate_log_old_newer_term() {
        let mut follower = new_node();
        follower.log.push(LogEntry {
            command: None,
            term: 1,
        });
        let req = VoteRequest {
            candidate_id: String::from("candidate"),
            term: 1,
            last_log_index: None,
            last_log_term: 0,
        };
        let expected_response = VoteResponse {
            term: 1,
            vote_granted: false,
        };
        test_request_vote(&mut follower, req, expected_response);
    }

    #[test]
    fn test_candidate_newer_term() {
        let mut follower = new_node();
        follower.state = State::Leader;
        let req = VoteRequest {
            candidate_id: String::from("candidate"),
            term: 2,
            last_log_index: None,
            last_log_term: 0,
        };
        let expected_response = VoteResponse {
            term: 2,
            vote_granted: true,
        };
        test_request_vote(&mut follower, req, expected_response);
        assert_eq!(follower.state, State::Follower);
        assert_eq!(follower.voted_for, Some(String::from("candidate")));
    }

    #[test]
    fn test_already_voted() {
        let mut follower = new_node();
        follower.voted_for = Some(String::from("other candidate"));
        let req = VoteRequest {
            candidate_id: String::from("candidate"),
            term: 1,
            last_log_index: None,
            last_log_term: 0,
        };
        let expected_response = VoteResponse {
            term: 1,
            vote_granted: false,
        };
        test_request_vote(&mut follower, req, expected_response);
    }

    #[test]
    fn test_leader_replaced() {
        let mut follower = new_node();
        follower.voted_for = Some(String::from("other candidate"));
        let req = VoteRequest {
            candidate_id: String::from("candidate"),
            term: 2,
            last_log_index: None,
            last_log_term: 0,
        };
        let expected_response = VoteResponse {
            term: 2,
            vote_granted: true,
        };
        test_request_vote(&mut follower, req, expected_response);
        assert_eq!(follower.voted_for, Some(String::from("candidate")));
    }

    fn test_append_entries(
        receiver: &mut Node,
        req: AppendEntriesRequest,
        expected: AppendEntriesResponse,
    ) {
        match receiver.append_entries(tonic::Request::new(req)) {
            Err(e) => println!("unexpected request_vote error: {}", e),
            Ok(res) => {
                assert_eq!(res.into_inner(), expected);
            }
        }
    }

    fn noop_entry(term: u64) -> LogEntry {
        return LogEntry {
            command: None,
            term: term,
        };
    }

    #[test]
    fn test_ae_heartbeat() {
        let mut follower = new_node();
        let req = AppendEntriesRequest {
            leader_id: String::from("leader"),
            term: 1,
            prev_log_index: None,
            prev_log_term: 0,
            entries: Vec::new(),
            leader_commit: 0,
        };
        let expected_response = AppendEntriesResponse {
            term: 1,
            success: true,
        };
        test_append_entries(&mut follower, req, expected_response);
    }

    #[test]
    fn test_ae_older_term() {
        let mut follower = new_node();
        follower.current_term = 2;
        let req = AppendEntriesRequest {
            leader_id: String::from("leader"),
            term: 1,
            prev_log_index: None,
            prev_log_term: 0,
            entries: Vec::from([noop_entry(1)]),
            leader_commit: 0,
        };
        let expected_response = AppendEntriesResponse {
            term: 2,
            success: false,
        };
        test_append_entries(&mut follower, req, expected_response);
    }

    #[test]
    fn test_ae_inconsistent() {
        let mut follower = new_node();
        let req = AppendEntriesRequest {
            leader_id: String::from("leader"),
            term: 1,
            prev_log_index: Some(0),
            prev_log_term: 1,
            entries: Vec::from([noop_entry(1)]),
            leader_commit: 0,
        };
        let expected_response = AppendEntriesResponse {
            term: 1,
            success: false,
        };
        test_append_entries(&mut follower, req, expected_response);
    }

    #[test]
    fn test_ae_initial() {
        let mut follower = new_node();
        let leader_entries = Vec::from([noop_entry(1), noop_entry(1), noop_entry(1)]);
        let req = AppendEntriesRequest {
            leader_id: String::from("leader"),
            term: 1,
            prev_log_index: None,
            prev_log_term: 0,
            entries: leader_entries,
            leader_commit: 0,
        };
        let expected_response = AppendEntriesResponse {
            term: 1,
            success: true,
        };
        test_append_entries(&mut follower, req, expected_response);
        let expected_log = Vec::from([noop_entry(1), noop_entry(1), noop_entry(1)]);
        assert_eq!(follower.log, expected_log);
    }

    #[test]
    fn test_ae_overwrite() {
        let mut follower = new_node();
        let follower_log = Vec::from([noop_entry(1), noop_entry(2), noop_entry(3)]);
        follower.log = follower_log;
        let leader_entries = Vec::from([noop_entry(4), noop_entry(4), noop_entry(4)]);
        let req = AppendEntriesRequest {
            leader_id: String::from("leader"),
            term: 4,
            prev_log_index: Some(1),
            prev_log_term: 2,
            entries: leader_entries,
            leader_commit: 0,
        };
        let expected_response = AppendEntriesResponse {
            term: 4,
            success: true,
        };
        let leader_id = req.clone().leader_id;
        test_append_entries(&mut follower, req, expected_response);
        let expected_log = Vec::from([
            noop_entry(1),
            noop_entry(2),
            noop_entry(4),
            noop_entry(4),
            noop_entry(4),
        ]);
        assert_eq!(follower.log, expected_log);
        assert_eq!(follower.voted_for, Some(leader_id));
    }
    #[tokio::test]
    async fn test_follower() {
        let mut f = new_node();
        f.start_follower().await;
        assert_eq!(f.state, State::Candidate);
    }
}
