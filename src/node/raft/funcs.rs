use std::{
    cmp,
    collections::HashMap,
    time::{Duration, Instant},
};

use futures::{executor::block_on, future::select_all};
use rand::Rng;
use tokio::sync::{mpsc, oneshot};
use tonic::{Request, Response, Status};

use crate::node::raft_rpcs::{
    raft_rpc_client::RaftRpcClient, AppendEntriesRequest, AppendEntriesResponse, Command,
    GetRequest, GetResponse, LogEntry, PutRequest, PutResponse, VoteRequest, VoteResponse,
};

use super::{Event, Node, ServerConfig, State};

#[allow(dead_code)]
impl Node {
    /// Returns a new follower node
    ///
    /// # Arguments
    /// * `id` - String used to name of node
    /// * `peers` - Vector of other nodes in the cluster
    /// * `mailbox` - Mailbox to collect incoming requests from other nodes
    ///

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
                timeout: Duration::new(2, 0),
                batch_size: 5,
                batch_timeout: Duration::new(5, 0),
            },
            log: Vec::new(),
            mailbox: mailbox,
            connections: HashMap::new(),
            batched_put_requests: Vec::new(),
            batched_get_requests: Vec::new(),
            batch_put_timeout: Some(Instant::now()),
            batch_get_timeout: Some(Instant::now()),
            batched_put_senders: Vec::new(),
            batched_get_senders: Vec::new(),
        }
    }

    /// Copies command into log of node
    ///
    /// # Arguments
    /// * `self` - Node recieving the request
    /// * `command` - Request being recieved
    ///
    /// # Explanation
    ///
    /// Once a leader has been elected, it begins servicing
    /// client requests. Each client request contains a command to
    /// be executed by the replicated state machines. The leader
    /// appends the command to its log as a new entry, then issues
    /// AppendEntries RPCs in parallel to each of the other
    /// servers to replicate the entry. When the entry has been
    /// safely replicated (as described below), the leader applies
    /// the entry to its state machine and returns the result of that
    /// execution to the client.
    ///
    /// A log entry is committed once the leader
    /// that created the entry has replicated it on a majority of
    /// the servers (e.g., entry 7 in Figure 6)
    ///
    async fn replicate(
        &mut self,
        command: Option<Command>,
    ) -> Result<Response<PutResponse>, Status> {
        self.log.push(LogEntry {
            command: command,
            term: self.current_term,
        });
        println!("{} is replicating command", self.id);
        // let mut requests = Vec::new();
        // let mut responses = Vec::new();
        // for peer in &self.peers {
        //     let (prev_log_index, prev_log_term) = match self.next_index.get(peer) {
        //         None => (None, 0),
        //         Some(a) => (Some(*a), self.log[*a as usize].term),
        //     };
        //     let mut client = match RaftClient::connect(peer.clone()).await {
        //         Err(_) => continue,
        //         Ok(c) => c,
        //     };
        //     let request = AppendEntriesRequest {
        //         term: self.current_term,
        //         leader_id: self.id.clone(),
        //         entries: Vec::new(),
        //         leader_commit: self.commit_index,
        //         prev_log_index: prev_log_index,
        //         prev_log_term: prev_log_term,
        //     };
        //     responses.push(tokio::spawn(async move {
        //         client.append_entries(Request::new(request)).await
        //     }));
        //     requests.push((peer.clone(), request));
        // }
        // while !responses.is_empty() {
        //     match select_all(responses).await {
        //         (Ok(Ok(res)), i, remaining) => {
        //             let r = res.into_inner();
        //             if !r.success {
        //                 let mut client = match RaftClient::connect(requests[i].0.clone()).await {
        //                     Err(_) => continue,
        //                     Ok(c) => c,
        //                 };
        //                 let req = &requests[i].1;
        //                 // req.prev_log_index = match req.prev_log_index {
        //                 //     None => None,
        //                 //     Some(0) => None,
        //                 //     Some(i) => Some(i - 1),
        //                 // };
        //                 responses.push(tokio::spawn(async move {
        //                     client
        //                         .append_entries(Request::new(requests[i].1.clone()))
        //                         .await
        //                 }));
        //             }
        //             if r.term > self.current_term {
        //                 self.current_term = r.term;
        //                 self.state = State::Follower;
        //                 self.voted_for = None;
        //                 return Ok(Response::new(PutResponse {
        //                     success: false,
        //                     leader_id: None,
        //                 }));
        //             }
        //             responses = remaining
        //         }
        //         (_, _, remaining) => responses = remaining,
        //     }
        // }

        Err(Status::unimplemented("not implemented"))
    }

    /// Recieves client request, verifies node's leadership by exchanging heartbeat
    /// to rest of cluster, then sends success or failure back to client
    ///
    /// # Arguments
    /// `self` - Node recieving the client request
    /// `req` - Get request sent by client
    /// `tx` - GRPC transaction
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
                .unwrap_or_else(|_| ());
        }
        let mut responses = Vec::new();
        for peer in &self.peers {
            // exchange heartbeats with majority of cluster
            let (prev_log_index, prev_log_term) = match self.next_index.get(peer) {
                None => (None, 0),
                Some(a) => (Some(*a), self.log[*a as usize].term),
            };
            let mut client = match RaftRpcClient::connect(peer.clone()).await {
                Err(_) => continue,
                Ok(c) => c,
            };
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
                                .unwrap_or_else(|_| ());
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
                            .unwrap_or_else(|_| ());
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
            .unwrap_or_else(|_| ());
    }

    /// Sends command to rest of cluster
    ///
    /// # Arguments
    /// `self` - Node sending request
    /// `req` - Put request being sent
    /// `tx` - GRPC Transaction
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
            .unwrap_or_else(|_| ());
            return;
        }
        let command = Command {
            key: req.key,
            value: req.value,
        };
        tx.send(self.replicate(Some(command)).await)
            .unwrap_or_else(|_| ());
    }

    /// Handles events received by node
    ///
    /// # Arguments
    /// `self` - Node receiving event
    /// `event` - Event being
    async fn handle_event(&mut self, event: Event) {
        match event {
            Event::RequestVote { req, tx } => tx
                .send(self.handle_request_vote(tonic::Request::new(req)))
                .unwrap_or_else(|_| ()),
            Event::AppendEntries { req, tx } => tx
                .send(self.handle_append_entries(tonic::Request::new(req)))
                .unwrap_or_else(|_| ()),
            Event::ClientGetRequest { req, tx } => return self.handle_get_request(req, tx).await,
            Event::ClientPutRequest { req, tx } => return self.handle_put_request(req, tx).await,
        }
    }

    pub fn refresh_timeout(self: &mut Self) {
        let mut rng = rand::thread_rng();
        let num = rng.gen_range(300..1000);
        self.next_timeout = Some(Instant::now() + Duration::from_millis(num));
    }

    pub fn timed_out(self: &mut Self) -> bool {
        match self.next_timeout {
            Some(t) => Instant::now() > t,
            None => false,
        }
    }

    async fn start_follower(&mut self) {
        self.refresh_timeout();
        println!("starting follower");
        loop {
            if self.timed_out() {
                self.state = State::Candidate;
                return;
            }
            match self.mailbox.try_recv() {
                Ok(event) => self.handle_event(event).await,
                _ => continue,
            }
        }
    }

    /// [start_candidate] starts a new node with candidate and sends RequestVoteRPCs to all peer nodes. Precondition: self.state == candidate
    async fn start_candidate(&mut self) {
        loop {
            if self.state != State::Candidate {
                return;
            }
            println!("starting candidate");
            self.current_term += 1;
            self.voted_for = Some(self.id.clone());
            let mut responses = Vec::new();
            for peer in &self.peers {
                let mut client = match RaftRpcClient::connect(peer.clone()).await {
                    Err(_) => continue,
                    Ok(c) => c,
                };
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
                request.set_timeout(Duration::from_secs(1));
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
            loop {
                match self.mailbox.try_recv() {
                    Ok(event) => self.handle_event(event).await,
                    _ => {
                        let mut rng = rand::thread_rng();
                        let num = rng.gen_range(300..1000);
                        tokio::time::sleep(Duration::from_millis(num)).await;
                        break;
                    }
                }
            }
        }
    }

    async fn send_heartbeat(&mut self) {
        if self.state != State::Leader {
            return;
        }
        let mut responses = Vec::new();
        for peer in &self.peers {
            let (prev_log_index, prev_log_term) = match self.next_index.get(peer) {
                None => (None, 0),
                Some(a) => (Some(*a), self.log[*a as usize].term),
            };
            let mut client = match RaftRpcClient::connect(peer.clone()).await {
                Err(_) => continue,
                Ok(c) => c,
            };
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
        while !responses.is_empty() {
            match select_all(responses).await {
                (Ok(Ok(res)), _, remaining) => {
                    let r = res.into_inner();
                    if r.term > self.current_term {
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
    }

    async fn start_leader(&mut self) {
        println!("starting leader in term {:?}", self.current_term);
        // commit no-op entry at the start of term
        // self.replicate(None);
        loop {
            if self.state != State::Leader {
                return;
            }

            tokio::time::sleep(Duration::from_millis(50)).await;
            self.send_heartbeat().await;
        }
    }

    pub async fn start(&mut self) {
        // Sets up connection to all peer nodes (panic if it can't find the connection - fix later :D)
        let mut connections = HashMap::new();
        self.peers.clone().into_iter().for_each(|ip| {
            println!("Connecting to {}", ip);
            connections.insert(ip.clone(), block_on(RaftRpcClient::connect(ip)).unwrap());
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

    /// checks if a log entry (log_term, log_index) exceeds the current node's log term and index
    fn log_newer_than(&self, log_term: u64, log_index: Option<u64>) -> bool {
        match log_index {
            None => self.log.len() > 0,
            Some(i) => {
                if self.log.len() == 0 {
                    return false;
                }
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

    pub fn handle_request_vote(
        &mut self,
        request: Request<VoteRequest>,
    ) -> Result<Response<VoteResponse>, Status> {
        // Respond to a Vote Request.
        // Sends a failure response if request is from a stale term or if node has already voted
        let req = request.into_inner();
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
    pub fn handle_append_entries(
        &mut self,
        request: Request<AppendEntriesRequest>,
    ) -> Result<Response<AppendEntriesResponse>, Status> {
        // Responds to an Append Entries Request.
        // Sends a failure response if request is from a stale term
        // Otherwise, checks log consistency and adds missing entries
        let req = request.into_inner();
        if req.term < self.current_term {
            return self.respond_to_ae(false);
        }
        self.refresh_timeout();
        if req.term > self.current_term {
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

    ////////////////////////////////////////////////////////////////////////////
    // Batching below
    ////////////////////////////////////////////////////////////////////////////

    /// Send out batch when time is up or batch is filled
    async fn start_batched_put(&mut self) {
        self.batch_put_timeout = Some(Instant::now());
        loop {
            let timeout = match self.batch_put_timeout {
                Some(t) => t.elapsed() >= self.config.batch_timeout,
                None => false,
            };
            if self.batched_put_requests.len() > self.config.batch_size || timeout {
                let log = Vec::<Command>::new();
                for req in self.batched_put_requests {
                    let command = Command {
                        key: req.key,
                        value: req.value,
                    };
                    log.push(command)
                }

                // Change replicate to take in Vec<Command> instead of just Command
                let result = self.replicate(log).await;

                for tx in self.batched_put_senders {
                    tx.send(result).unwrap_or_else(|_| ());
                }

                self.batched_put_requests = Vec::<PutRequest>::new();
                self.batched_put_senders =
                    Vec::<oneshot::Sender<Result<Response<PutResponse>, Status>>>::new();
                self.batch_put_timeout = Some(Instant::now());
            }
        }
    }

    /// Add put requests to batch
    async fn handle_batched_put_request(
        &mut self,
        req: PutRequest,
        tx: oneshot::Sender<Result<Response<PutResponse>, Status>>,
    ) {
        if self.state != State::Leader {
            tx.send(Ok(Response::new(PutResponse {
                success: false,
                leader_id: self.voted_for.clone(),
            })))
            .unwrap_or_else(|_| ());
            return;
        }

        self.batched_put_requests.push(req);
        self.batched_put_senders.push(tx);
    }

    /// Send out get request when time limit is exceeded or batch size is hit
    async fn start_batched_get(&mut self) {
        self.batch_get_timeout = Some(Instant::now());
        loop {
            let timeout = match self.batch_get_timeout {
                Some(t) => t.elapsed() >= self.config.batch_timeout,
                None => false,
            };
            if self.batched_get_requests.len() > self.config.batch_size || timeout {
                while self.batched_get_senders.len() > 0 {
                    let req = match self.batched_get_requests.pop() {
                        Some(a) => a,
                        None => break,
                    };
                    let tx = match self.batched_get_senders.pop() {
                        Some(a) => a,
                        None => break,
                    };
                    self.handle_get_request(req, tx);
                }
            }
        }
    }

    /// Add get requests to batch
    async fn handle_batched_get_request(
        &mut self,
        req: GetRequest,
        tx: oneshot::Sender<Result<Response<GetResponse>, Status>>,
    ) {
        if self.state != State::Leader {
            tx.send(Ok(Response::new(GetResponse {
                value: 0,
                success: false,
                leader_id: self.voted_for.clone(),
            })))
            .unwrap_or_else(|_| ());
            return;
        }

        self.batched_get_requests.push(req);
        self.batched_get_senders.push(tx);
    }
}

#[cfg(test)]
#[path = "./tests.rs"]
mod tests;
