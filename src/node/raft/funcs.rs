use std::{collections::HashMap, time::{Duration, Instant}, cmp};

use futures::{future::select_all, executor::block_on};
use rand::Rng;
use tokio::sync::{mpsc, oneshot};
use tonic::{Response, Status, Request};

use crate::node::raft_rpcs::{GetRequest, GetResponse, raft_rpc_client::RaftRpcClient, AppendEntriesRequest, PutRequest, PutResponse, Command, VoteRequest, VoteResponse, AppendEntriesResponse, LogEntry};

use super::{State, Event, ServerConfig, Node};


#[allow(dead_code)]
impl Node {
    /// Returns a new node, initialized as a follower
    ///
    /// # Arguments
    /// * `id` - Ip address of the node
    /// * `state` - The state of the node in the Raft algorithm, either Follower, Candidate, or Leader
    /// * `commit_index` - The highest index log entry on the node that has been committed by the leader
    /// * `last_applied` - The highest index of the log entry applied to the state machine
    /// * `peers` - Vector of ip addresses of other nodes in the cluster
    /// * `state_machine` - TODO: The game
    /// * `current_term` - The current term known by the node
    /// * `voted_for` - The ip address the current node believes to be the leader/hopes to elect as leader
    /// * `voted` - Whether the node has voted in the current term
    /// * `log` - Vector of log entries
    /// * `connections`
    /// * `next_timeout`
    /// * `config`
    /// * `mailbox` - Mailbox to collect incoming requests from other nodes
    /// ## Leader Only
    /// * `next_index` - The index of the next log entry a leader should send to the follower
    /// * `match_index` - The highest index log entry where a follower is consistent with the leader

    pub fn new(id: String, peers: Vec<String>, mailbox: mpsc::UnboundedReceiver<Event>) -> Self {
        Self {
            id: id,
            state: State::Follower,
            commit_index: 0,
            last_applied: 0,
            peers: peers,
            state_machine: HashMap::new(),
            current_term: 1,
            voted_for: None,
            voted: false,
            log: Vec::new(),
            mailbox: mailbox,
            connections: HashMap::new(),
            next_timeout: None,
            config: ServerConfig {
                timeout: Duration::new(2, 0),
            },
            next_index: HashMap::new(),
            match_index: HashMap::new(),
        }
    }

    // ------------------------------- HELPERS --------------------------------

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

    fn get_entries(&self, from_term : &u64, from_idx : &u64) -> Vec<LogEntry> {
        self.log.iter().enumerate().filter({| (index, l) |
            l.term > *from_term && *index > (*from_idx).try_into().unwrap()
        }).into_iter().map( | (_index, l) |
            l.clone()
        ).collect()
    }

    /// returns whether the current node has a more up-to-date log than (log_term, log_index)
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

    // if voted, then voted_for is the candidate the node voted for (or the leader, if it was elected)
    // if not voted, then voted_for is the leader from the previous term
    // if not voted and gets a valid vote request, update voted and voted_for to the candidate
    // and once voted, cannot vote again in the given term
    // to_follower occurs when a node finds out another node has a higher term
    fn to_follower(&mut self, term: u64) {
        // TODO: should this also refresh timeouts?
        self.current_term = term;
        self.state = State::Follower;
        self.voted = false;
    }

    // ------------------------------- CLIENT REQUESTS ------------------------

    /// Recieves client request, verifies node's leadership by exchanging heartbeat
    /// to rest of cluster, then sends success or failure back to client
    ///
    /// # Arguments
    /// `self` - Node recieving the client request
    /// `req` - Request sent by client
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
            let prev_log_index = self.next_index.get(peer).unwrap();
            let prev_log_term = self.log[*prev_log_index as usize].term;

            let mut client = match RaftRpcClient::connect(peer.clone()).await {
                Err(_) => continue,
                Ok(c) => c,
            };
            let request = Request::new(AppendEntriesRequest {
                term: self.current_term,
                leader_id: self.id.clone(),
                entries: Vec::new(),
                leader_commit: self.commit_index,
                prev_log_index: *prev_log_index,
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
                        self.to_follower(r.term);
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
    /// `req` - Request being sent
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

    // --------------------------- APPEND ENTRIES -----------------------------

    fn gen_ae_request(&self, peer: &String, heartbeat: bool) -> Request<AppendEntriesRequest> {
        let prev_log_index = self.next_index.get(peer).unwrap();
        let prev_log_term = self.log[*prev_log_index as usize].term;

        let entries = if heartbeat { Vec::new() }
            else { self.get_entries(&prev_log_term, prev_log_index) };

        Request::new(AppendEntriesRequest {
            term: self.current_term,
            leader_id: self.id.clone(),
            entries: entries,
            leader_commit: self.commit_index,
            prev_log_index: *prev_log_index,
            prev_log_term: prev_log_term,
        })
    }

    async fn send_append_entries(&mut self) {
        if self.state != State::Leader { return; }
        let mut responses = Vec::new();
        for peer in &self.peers {
            let mut client = match RaftRpcClient::connect(peer.clone()).await {
                Err(_) => continue,
                Ok(c) => c,
            };

            let request = self.gen_ae_request(peer, false);

            let p = peer.clone();
            let commit_idx = self.commit_index.clone();
            responses.push(tokio::spawn(
                async move { 
                    (p, commit_idx, client.append_entries(request).await)
                },
            ));

        }
        while !responses.is_empty() {
            match select_all(responses).await {
                (Ok((responder, last_sent_idx, Ok(res))), _, remaining) => {
                    let r = res.into_inner();
                    if r.term > self.current_term {
                        self.to_follower(r.term);
                        return;
                    }
                    // update matchIndex and nextIndex if successful
                    // both should be the last entry that was sent in request
                    if r.success {
                        self.match_index.entry(responder.to_string()).or_insert(last_sent_idx);
                        self.next_index.entry(responder.to_string()).or_insert(last_sent_idx);
                    } else {
                        // failed due to log inconsistency
                        // set nextindex to mismatch index
                        self.next_index.entry(responder.to_string()).or_insert(r.mismatch_index);
                    }
                    responses = remaining
                }
                (_, _, remaining) => responses = remaining,
            }
        }
    }

    async fn send_heartbeat(&mut self) {
        if self.state != State::Leader {
            return;
        }
        let mut responses = Vec::new();
        for peer in &self.peers {
            let mut client = match RaftRpcClient::connect(peer.clone()).await {
                Err(_) => continue,
                Ok(c) => c,
            };

            let request = self.gen_ae_request(peer, true);

            responses.push(tokio::spawn(
                async move { client.append_entries(request).await },
            ));
        }
        while !responses.is_empty() {
            match select_all(responses).await {
                (Ok(Ok(res)), _, remaining) => {
                    let r = res.into_inner();
                    if r.term > self.current_term {
                        self.to_follower(r.term);
                        return;
                    }
                    responses = remaining
                }
                (_, _, remaining) => responses = remaining,
            }
        }
    }

    /// Generates a response for request to append entries.
    /// Failure response if request is from a stale term or the node's log is inconsistent.
    /// Locates log inconsistencies and fixes them.
    pub fn handle_append_entries(
        &mut self,
        request: Request<AppendEntriesRequest>,
    ) -> Result<Response<AppendEntriesResponse>, Status> {

        let req = request.into_inner();
        if req.term < self.current_term {
            // mismatch_index is unused in this case
            return self.respond_to_ae(false, self.log.len());
        }
        // when receiving AE from current leader (node of same or higher term)
        // revert to follower state, refresh timeout, and update known leader
        self.refresh_timeout();
        self.state = State::Follower;
        self.voted = true;
        self.voted_for = Some(req.leader_id);

        // heartbeat: just respond immediately
        if req.entries.is_empty() {
            return self.respond_to_ae(true, self.log.len());
        }
        // next_i is the index of the first new entry to add to the log
        // consistent (at least up until prev_log_term) if log is at least as many entries
        // and the term matches (can prove inductively?) 
        let i = req.prev_log_index as usize;
        // if log missing entries, return log.len as mismatch index
        // if not missing entries, then inconsistent starting from where term mismatch (prev log index)
        // meaning leader should decrement next_index[peer]
        let missing = !(i < self.log.len());
        let (is_consistent, mismatch_idx) =
            if missing {
                (false, self.log.len())
            } else if self.log[i].term != i.try_into().unwrap() {
                (false, i)
            } else {
                (true, self.log.len())
            };
        if !is_consistent {
            return self.respond_to_ae(is_consistent, mismatch_idx)
        }

        // add entries, replacing entries if node already has an entry at index
        let n = req.entries.len();
        let next_i = i + 1;
        for j in 0..n {
            if next_i + j < self.log.len() {
                self.log[next_i + j] = req.entries[j].clone();
                continue;
            }
            self.log.push(req.entries[j].clone());
        }

        // update commit index
        if req.leader_commit > self.commit_index {
            self.commit_index = cmp::min(req.leader_commit, (self.log.len() - 1) as u64);
        }

        return self.respond_to_ae(true, self.log.len());
    }

    fn respond_to_ae(&self, success: bool, mismatch_index: usize) -> Result<Response<AppendEntriesResponse>, Status> {
        Ok(Response::new(AppendEntriesResponse {
            term: self.current_term,
            success: success,
            mismatch_index: mismatch_index.try_into().unwrap(),
        }))
    }

    // ----------------------------- VOTE REQUEST -----------------------------

    async fn send_vote_request(&mut self) {
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
                            self.to_follower(r.term);
                            return;
                        }
                        responses = remaining
                    }
                    (_, _, remaining) => responses = remaining,
                }
            }
    }

    /// Generate response to a Vote Request.
    /// Sends a failure response if request is from a stale term, log out of date, or node already voted
    pub fn handle_request_vote(
        &mut self,
        request: Request<VoteRequest>,
    ) -> Result<Response<VoteResponse>, Status> {
        let req = request.into_inner();

        // stale term check
        if req.term < self.current_term { return self.respond_to_vote(false); }

        // when receives higher term vote req, follower updates term and voted status for a fresh election
        // could receive reqs in a similar term, if multiple cands are running for election
        // in both cases, only vote if hasn't voted yet and cand is more up to date
        if req.term > self.current_term { self.to_follower(req.term); }
        if self.voted || self.log_newer_than(req.last_log_term, req.last_log_index) {
            return self.respond_to_vote(false);
        }

        // refresh timeout upon granting vote to candidate
        self.refresh_timeout();
        self.voted_for = Some(req.candidate_id);
        self.voted = true;
        return self.respond_to_vote(true);
    }

    fn respond_to_vote(&self, vote_granted: bool) -> Result<Response<VoteResponse>, Status> {
        Ok(Response::new(VoteResponse {
            term: self.current_term,
            vote_granted: vote_granted,
        }))
    }

    // -------------------------------- MAIN LOOP -----------------------------

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
            Event::ClientGetRequest { req, tx } =>
                return self.handle_get_request(req, tx).await,
            Event::ClientPutRequest { req, tx } =>
                return self.handle_put_request(req, tx).await,
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
            self.send_vote_request().await;
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

}

#[cfg(test)]
#[path = "./tests.rs"]
mod tests;