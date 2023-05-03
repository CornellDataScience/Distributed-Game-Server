use std::{
    cmp,
    collections::{BTreeMap, HashMap},
    time::{Duration, Instant},
};

use futures::{executor::block_on, future::select_all};
use json::JsonValue;
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
    /// Returns a new node, initialized as a follower
    ///
    /// # Arguments
    /// * `id` - Ip address of the node
    /// * `state` - The state of the node in the Raft algorithm, either Follower, Candidate, or Leader
    /// * `commit_index` - The highest index log entry on the node that has been committed by the leader
    /// * `last_applied` - The highest index of the log entry applied to the state machine
    /// * `peers` - Vector of ip addresses of other nodes in the cluster
    /// * `state_machine` - TODO: The game
    /// * `current_term` - The current term known by the node. Starts at 1
    /// * `voted_for` - The ip address the current node believes to be the leader/hopes to elect as leader
    /// * `voted` - Whether the node has voted in the current term
    /// * `log` - Vector of log entries
    /// * `connections`
    /// * `next_timeout`
    /// * `config`
    /// * `mailbox` - Mailbox to collect incoming requests from other nodes
    /// ## Leader Only
    /// * `next_index` - The index of the next log entry a leader should send to the follower.
    ///        in AE, prev_log_index will be None if leader had never sent an AE to the follower
    /// * `match_index` - The highest index log entry where a follower is consistent with the leader

    pub fn new(id: String, mailbox: mpsc::UnboundedReceiver<Event>) -> Self {
        Self {
            id: id,
            state: State::Follower,
            commit_index: 0,
            last_applied: 0,
            peers: vec![],
            state_machine: HashMap::new(),
            current_term: 1,
            voted_for: None,
            voted: false,
            // dummy entry to make computations easier
            log: vec![LogEntry {
                command: Some(Command {
                    data: r#"{"$" : 0 }"#.to_string(),
                }),
                term: 0,
            }],
            mailbox: mailbox,
            connections: HashMap::new(),
            next_timeout: None,
            config: ServerConfig {
                timeout: Duration::new(2, 0),
                batch_size: 5,
                batch_timeout: Duration::new(5, 0),
            },
            batched_put_requests: Vec::new(),
            batched_get_requests: Vec::new(),
            batch_put_timeout: Some(Instant::now()),
            batch_get_timeout: Some(Instant::now()),
            batched_put_senders: Vec::new(),
            batched_get_senders: Vec::new(),
            next_index: HashMap::new(),
            match_index: HashMap::new(),
        }
    }

    pub fn set_peers(&mut self, peers: Vec<String>) {
        self.peers = peers;
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

    /// returns a vector of log entries which have index and term greater than from_idx and from_term
    fn get_entries(&self, from_term: &u64, from_idx: &u64) -> Vec<LogEntry> {
        self.log
            .iter()
            .enumerate()
            .filter({
                |(index, l)| l.term > *from_term && *index > (*from_idx).try_into().unwrap()
            })
            .into_iter()
            .map(|(_index, l)| l.clone())
            .collect()
    }

    /// returns whether the current node has a more up-to-date log than (log_term, log_index)
    fn log_newer_than(&self, log_term: u64, log_index: u64) -> bool {
        let last_index = self.log.len() - 1;
        let last_term = self.log[last_index].term;
        last_term > log_term || (last_term == log_term && last_index as u64 > log_index)
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

    fn print_debug(&self) {
        // print log, term, state machine
        println!("Log entries: {:?}", self.log);

        println!("curr term {}", self.current_term);
        println!("commit index {}", self.commit_index);
        println!("State Machine Applied Entries");
        println!("{:?}", self.state_machine);
        if self.state == State::Leader {
            println!("ip, next index, match index");
            for id in &self.peers {
                println!(
                    "{}, {}, {}",
                    id.clone(),
                    self.next_index.get(&id.clone()).unwrap(),
                    self.match_index.get(&id.clone()).unwrap()
                );
            }
        }
    }

    // ------------------------------- CLIENT REQUESTS ------------------------

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
                    value: json::stringify(0),
                    success: false,
                    leader_id: self.voted_for.clone(),
                })))
                .unwrap_or_else(|_| ());
        }
        let mut responses = Vec::new();
        for peer in &self.peers {
            // exchange heartbeats with majority of cluster
            let mut client = match RaftRpcClient::connect(peer.clone()).await {
                Err(_) => continue,
                Ok(c) => c,
            };
            let request = self.gen_ae_request(peer, true);

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
                            println!("majority replicated");
                            println!("{:?}", self.state_machine);
                            return tx
                                .send(Ok(Response::new(GetResponse {
                                    value: (*self
                                        .state_machine
                                        .get(&req.key)
                                        .unwrap_or(&json::JsonValue::Null))
                                    .to_string(),
                                    success: true,
                                    leader_id: Some(self.id.clone()),
                                })))
                                .unwrap_or_else(|_| ());
                        }
                    } else if r.term > self.current_term {
                        self.to_follower(r.term);
                        return tx
                            .send(Ok(Response::new(GetResponse {
                                value: json::stringify(0),
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
        tx.send(Ok(Response::new(GetResponse {
<<<<<<< HEAD
            value: 0,
=======
            value: json::stringify(0),
>>>>>>> main
            success: false,
            leader_id: Some(self.id.clone()),
        })))
        .unwrap_or_else(|_| ())
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
            println!("rerouting put req");
            tx.send(Ok(Response::new(PutResponse {
                success: false,
                leader_id: self.voted_for.clone(),
            })))
            .unwrap_or_else(|_| ());
            return;
        }
<<<<<<< HEAD
        let command = Command {
            key: req.key,
            value: req.value,
        };
=======
        let command = Command { data: req.data };
>>>>>>> main
        tx.send(self.replicate(&vec![command]).await)
            .unwrap_or_else(|_| ());
    }

    /// parses jsonstr into the state machine
    fn add_json_entries(&mut self, jstr: &String) {
        let parsed = json::parse(jstr).unwrap();
        for (k, v) in parsed.entries() {
            self.state_machine.insert(k.to_string(), v.clone());
        }
    }

    // --------------------------- APPEND ENTRIES -----------------------------

    fn gen_ae_request(&self, peer: &String, heartbeat: bool) -> Request<AppendEntriesRequest> {
        let prev_log_index = self.match_index.get(peer).unwrap_or_else(|| &0);
        let prev_log_term = self.log[*prev_log_index as usize].term;

        let entries = if heartbeat {
            Vec::new()
        } else {
            self.get_entries(&prev_log_term, prev_log_index)
        };

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
        if self.state != State::Leader {
            return;
        }
        let mut responses = Vec::new();
        for peer in &self.peers {
            println!("sending ae req to peer {}", peer);
            let mut client = match RaftRpcClient::connect(peer.clone()).await {
                Err(_) => continue,
                Ok(c) => c,
            };

            let request = self.gen_ae_request(peer, false);

            let p = peer.clone();
            responses.push(tokio::spawn(async move {
                (p, client.append_entries(request).await)
            }));
        }
        while !responses.is_empty() {
            match select_all(responses).await {
                (Ok((responder, Ok(res))), _, remaining) => {
                    let r = res.into_inner();
                    println!("got {} ae response from {}", r.success, responder);
                    self.handle_ae_response(responder, r);
                    responses = remaining
                }
                (_, _, remaining) => responses = remaining,
            }
        }
    }

    fn handle_ae_response(&mut self, responder: String, r: AppendEntriesResponse) {
        if r.term > self.current_term {
            self.to_follower(r.term);
            return;
        }
<<<<<<< HEAD
        // TODO: I think we don't need nextIndex if we just send the entries 
=======
        // TODO: I think we don't need nextIndex if we just send the entries
>>>>>>> main
        // between matching index and end of leader log. Might be an issue if nodes are
        // missing a significant number of entries though
        // in this case may want to send no more than some number of entries.

        // update matchIndex and nextIndex if successful
        // both should be the last entry that was sent in request
        if r.success {
            self.match_index
                .insert(responder.to_string(), self.log.len() as u64 - 1); // assume no entries added since sending request
            self.next_index
                .insert(responder.to_string(), self.log.len() as u64);
        } else {
            // failed due to log inconsistency
            // set nextindex to mismatch index
<<<<<<< HEAD
            self.match_index
                .entry(responder.to_string()).or_insert(0);
=======
            self.match_index.entry(responder.to_string()).or_insert(0);
>>>>>>> main
            self.next_index
                .insert(responder.to_string(), r.mismatch_index.unwrap());
        }
    }

    /// the leader node updates its commit idx to the highest index among the majority of its followers
    /// O(nlogn), where n is number of peers
    fn update_commit_idx(&mut self) {
        let n = self.peers.len();
        let maj = (n + 1) / 2;

        // create a map ordered by the keys (match indexes)
        let mut cumulative_occurrences = BTreeMap::new();
        for (_, i) in self.match_index.iter() {
            let o = cumulative_occurrences.entry(i).or_insert(0);
            *o += 1;
        }

        let mut highest_maj_idx = 0;
        let mut cum_sum = 0;
        // iterate over match idxs in descending order, stop when count is greater than majority
        for (idx, count) in cumulative_occurrences.iter().rev() {
            cum_sum += count;
            if cum_sum > maj && *idx > &highest_maj_idx {
                highest_maj_idx = **idx;
                break;
            }
        }
        if highest_maj_idx > self.commit_index
            && self.log[highest_maj_idx as usize].term == self.current_term
        {
            self.commit_index = highest_maj_idx;
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
            return self.respond_to_ae(false, None);
        }
        // when receiving AE from current leader (node of same or higher term)
        // revert to follower state, refresh timeout, and update known leader
        self.refresh_timeout();
        self.current_term = req.term;
        self.state = State::Follower;
        self.voted = true;
        self.voted_for = Some(req.leader_id);

        // heartbeat: just respond immediately
        if req.entries.is_empty() {
            return self.respond_to_ae(true, None);
        }

        println!("handling nonempty ae");

        // next_i is the index of the first new entry to add to the log
        // consistent (at least up until prev_log_term) if log is at least as many entries
        // and the term matches (can prove inductively?)
        let i = req.prev_log_index as usize;
        let t = req.prev_log_term;
        // if log missing entries, return log.len-1 as mismatch index
        // if not missing entries, then inconsistent starting from where term mismatch (prev log index)
        // meaning leader should decrement next_index[peer]
        let missing = i > self.log.len() - 1;
        let (is_consistent, mismatch_idx) = if missing {
            (false, Some(self.log.len()))
        } else if self.log[i].term != t {
            (false, Some(i))
        } else {
            (true, None)
        };
        if !is_consistent {
            return self.respond_to_ae(is_consistent, mismatch_idx);
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

        // remove entries past next_i+n, if they exist
        for _ in 0..(next_i + n - self.log.len()) {
            self.log.pop();
        }

        // update commit index and apply log entries to state machine
        if req.leader_commit > self.commit_index {
            println!("ae update commit and apply log entries");
            for i in self.commit_index + 1..req.leader_commit + 1 {
<<<<<<< HEAD
                let c = &self.log[i as usize].command;
                self.state_machine
                    .insert(c.as_ref().unwrap().key.clone(), c.as_ref().unwrap().value);
=======
                let c = self.log[i as usize].command.clone();
                self.add_json_entries(&c.as_ref().unwrap().data);
>>>>>>> main
            }
            self.commit_index = cmp::min(req.leader_commit, (self.log.len() - 1) as u64);
        }

        return self.respond_to_ae(true, None);
    }

    fn respond_to_ae(
        &self,
        success: bool,
        mismatch_index: Option<usize>,
    ) -> Result<Response<AppendEntriesResponse>, Status> {
        Ok(Response::new(AppendEntriesResponse {
            term: self.current_term,
            success: success,
            mismatch_index: match mismatch_index {
                None => None,
                Some(i) => Some(i as u64),
            },
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
            let (last_log_idx, last_term) = (
                (self.log.len() - 1) as u64,
                self.log[self.log.len() - 1].term,
            );
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
        if req.term < self.current_term {
            return self.respond_to_vote(false);
        }

        // when receives higher term vote req, follower updates term and voted status for a fresh election
        // could receive reqs in a similar term, if multiple cands are running for election
        // in both cases, only vote if hasn't voted yet and cand is more up to date
        if req.term > self.current_term {
            self.to_follower(req.term);
        }
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
        match &event {
            Event::ClientGetRequest { .. } | Event::ClientPutRequest { .. } => {
                println!("{:?}", event)
            }
            _ => {}
        }
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
            // TODO: If last log index ≥ nextIndex for a follower: send
            // AppendEntries RPC with log entries starting at nextIndex
            // If there exists an N such that N > commitIndex, a majority
            // of matchIndex[i] ≥ N, and log[N].term == currentTerm:
            // set commitIndex = N (§5.3, §5.4).

            // TODO: is this correct?
            // handle event if there is one, otherwise, heartbeat or continue updating followers
            match self.mailbox.try_recv() {
                Ok(event) => self.handle_event(event).await,
                // no event
                // if followers not up to date, send one round of AE
                // otherwise send heartbeat
                _ => {}
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
            // TODO: heartbeat or continue updating followers
            self.send_heartbeat().await;
            // so prints are slower
            // tokio::time::sleep(Duration::from_millis(5000)).await;
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
        commands: &Vec<Command>,
    ) -> Result<Response<PutResponse>, Status> {
        // If command received from client: append entry to local log,
        // respond after entry applied to state machine (§5.3)
        // which is once the command has been committed
        println!("calling replicate");
        // push entries onto log
        for command in commands {
            self.log.push(LogEntry {
                // does it make sense to use a clone here
                command: Some(command.clone()),
                term: self.current_term,
            });
        }

        // TODO: will this loop cause things to stall?
        let req_idx = self.log.len() - 1;
        // keep sending ae until req committed
        while self.commit_index < req_idx as u64 {
            // erroring here
            println!("{} is replicating command", self.id);
            self.send_append_entries().await;
            self.update_commit_idx();
            self.print_debug();
        }
        // apply new log entries to state machine
        for command in commands {
<<<<<<< HEAD
            self.state_machine.insert(
                command.key.clone(),
                command.value,
            );
=======
            self.add_json_entries(&command.data);
>>>>>>> main
        }

        Ok(Response::new(PutResponse {
            success: true,
            leader_id: Some(self.id.to_string()),
        }))
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
                let mut log = vec![];
                for req in &self.batched_put_requests {
                    let command = Command {
                        data: req.data.clone(),
                    };
                    log.push(command)
                }

                while !self.batched_put_senders.is_empty() {
<<<<<<< HEAD
                    let result: Result<Response<PutResponse>, Status> =
                        self.replicate(&log).await;
=======
                    let result: Result<Response<PutResponse>, Status> = self.replicate(&log).await;
>>>>>>> main
                    match self.batched_put_senders.pop() {
                        Some(tx) => tx.send(result).unwrap_or_else(|_| ()),
                        None => (),
                    }
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
                value: json::stringify(0),
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
