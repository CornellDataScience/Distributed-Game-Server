use crate::raft::log::Log;
use crate::raft::raft::{
    raft_client::RaftClient, AppendEntriesRequest, AppendEntriesResponse, LogEntry, VoteRequest,
    VoteResponse,
};
use futures::future::select_ok;
use std::{borrow::BorrowMut, cmp};
use std::{
    fs::{self, File},
    io::Write,
};
use tokio::sync::{mpsc, oneshot};
use tonic::{Request, Response, Status};

//use super::log::write_LogEntry;

//Write to log entries into persistent storage
pub fn write_LogEntry(path: &str, cache: Vec<LogEntry>) {
    let serialized = serde_json::to_string(&cache).unwrap();

    let mut new_path: String = "data/".to_owned();

    new_path.push_str(path);

    let mut file = File::create(new_path).expect("Error encountered while creating file!");
    file.write_all(&serialized.as_bytes())
        .expect("Unable to write file");
}

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
}

#[allow(dead_code)]
#[derive(Debug, Default, PartialEq)]
enum State {
    #[default]
    Follower,
    Candidate,
    Leader,
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
    state_machine: std::collections::HashMap<String, i64>,

    // persistent state
    current_term: u64,
    voted_for: Option<String>,
    log: Vec<LogEntry>,

    // volatile leader state
    next_index: Vec<u64>,
    match_index: Vec<u64>,

    // to receive messages from client or RPC server
    mailbox: mpsc::UnboundedReceiver<Event>,
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
            state_machine: std::collections::HashMap::new(),
            next_index: Vec::new(),
            match_index: Vec::new(),
            current_term: 1,
            voted_for: None,
            log: Vec::new(),
            mailbox: mailbox,
        }
    }

    fn handle_event(&mut self, event: Event) {
        match event {
            Event::RequestVote { req, tx } => {
                match tx.send(self.request_vote(tonic::Request::new(req))) {
                    Err(e) => println!("{:?}", e),
                    _ => (),
                }
            }
            Event::AppendEntries { req, tx } => {
                match tx.send(self.append_entries(tonic::Request::new(req))) {
                    Err(e) => println!("{:?}", e),
                    _ => (),
                }
            } /*
              Event::Put {
                  req, tx
              }
              => {
                  // tx here should be used to send the response back to the RPC
                  // Server
                  // here, leader wants to
                  match tx.send(self.broadcast_AE)
              }
               */
        }
    }

    // TODO: Not compiling
    pub async fn send_req_votes_conc(&mut self) {
        // send a request vote RPC to every peer in the network in parallel
        let p = self.peers.clone();

        let cand_id = self.id.clone();
        let curr_term = self.current_term.clone();
        let mut votes = 1; // need to handle how it votes for itself
        let maj = self.peers.len() / 2;

        for i in p {
            let cand_id = cand_id.clone();
            let curr_term = curr_term.clone();
            // construct message
            let request = tonic::Request::new(VoteRequest {
                term: curr_term,
                candidate_id: cand_id,
                last_log_index: Some(1), //index of candidate’s last log entry
                last_log_term: 1,
            }); //term of candidate’s last log LogEntry

            let mut connection = RaftClient::connect(String::from(i)).await.unwrap();
            let response = connection.request_vote(request).await.unwrap();
            // process

            // if term < response term, become a follower and return
            // otherwise,
            // if vote_granted increment votes.
            match response.into_inner() {
                vr => {
                    if vr.term > curr_term {
                        // do this atomically
                        self.state = State::Follower;
                        return;
                    } else if vr.vote_granted {
                        votes += 1;
                    }
                    if votes > maj {
                        self.state = State::Leader;
                        return;
                    }
                }
            }
        }
    }

    // async fn broadcast_AE(self) {
    //     // try to connect to clients (this should only happen once)
    //     // this is still kinda synchronous
    //     let clients = self.peers.iter()
    //     .map(|peer| RaftClient::connect(String::from(peer)));

    //     let connections =

    //     for client in clients {
    //         client.await;
    //     }
    //     // let rpc_params =
    //     // does the request look the same for all of them?
    //     // let request = gen_request(rpc_params)
    //     let responses = clients.map(|client| send_AE(client))

    //     // loop
    //     let (res, remaining) = select_ok(responses).await.unwrap();
    //     responses = remaining
    //     // process the response
    // }

    async fn start_follower(&mut self) {
        // TODO: implement follower loop: if timer runs out,
        // change state to candidate and return from function.
        println!("starting follower");
        loop {
            tokio::select! {
                Some(event) = self.mailbox.recv() => {
                    self.handle_event(event)
                }
            }
        }
    }

    async fn start_candidate(&mut self) {
        // TODO: implement candidate loop:
        //  1. Start election timer
        //  2. For each peer, establish connection, send RequestVoteRPC
        //       If peer is unreachable, treat it as a 'no' vote
        //  3. If majority of votes received, change state to
        //  4. leader and return from function

        println!("starting candidate");
        loop {
            tokio::select! {
                Some(event) = self.mailbox.recv() => {
                    self.handle_event(event)
                }
            }
        }
    }

    async fn start_leader(&mut self) {
        println!("starting leader");
        loop {
            tokio::select! {
                Some(event) = self.mailbox.recv() => {
                    self.handle_event(event)
                }
            }
        }
    }

    pub async fn start(&mut self) {
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

    pub fn append_entries(
        &mut self,
        request: Request<AppendEntriesRequest>,
    ) -> Result<Response<AppendEntriesResponse>, Status> {
        let req = request.into_inner();
        if req.term < self.current_term {
            return self.respond_to_ae(false);
        }
        // TODO: reset election timeout here
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

        write_LogEntry(&self.id.clone(), self.log.clone());

        return self.respond_to_ae(true);
    }
}

#[cfg(test)]
mod tests {
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

    #[test]
    fn test_send_vote_req() {
        let mut candidate = new_node();
        // timeout so node becomes candidate

        // make a follower
        let mut follower = new_node();

        // send out vote requests
        // candidate.send_req_votes_conc().await;

        // check transitioned to Follower if higher cand term
        // or transitioned to leader if majority vote
    }
}
