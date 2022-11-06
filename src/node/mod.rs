use raft::{
    raft_server::Raft, raft_server::RaftServer, AppendEntriesRequest, AppendEntriesResponse,
    LogEntry, VoteRequest, VoteResponse,
};
use std::cmp;
use std::io;
use std::sync::Arc;
use std::sync::Mutex;
use tonic::{Request, Response, Status};

#[cfg(test)]
mod tests;

pub mod raft {
    tonic::include_proto!("raft");
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
#[derive(Debug, Default)]
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
    log: Log,

    // volatile leader state
    next_index: Vec<u64>,
    match_index: Vec<u64>,
}

type Log = Vec<LogEntry>;
type RaftNode = Arc<Mutex<Node>>;

fn new_node(id: &str, peers: Vec<String>) -> RaftNode {
    Arc::new(Mutex::new(Node::new(id, peers)))
}

impl Node {
    pub fn new(id: &str, peers: Vec<String>) -> Self {
        Self {
            id: String::from(id),
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
            // voted_for : match voted_for {
            //     Some(p) => p,
            //     None => ""
            // }
        }))
    }

    fn respond_to_ae(&self, success: bool) -> Result<Response<AppendEntriesResponse>, Status> {
        Ok(Response::new(AppendEntriesResponse {
            term: self.current_term,
            success: success,
        }))
    }
}

#[tonic::async_trait]
impl Raft for RaftNode {
    async fn request_vote(
        &self,
        request: Request<VoteRequest>,
    ) -> Result<Response<VoteResponse>, Status> {
        let self = &mut self.lock().unwrap();
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
        println!("Vote for candidate (yes/no)?:");
        loop {
            let mut input = String::new();
            let stdin = io::stdin();
            stdin.read_line(&mut input)?;
            input.truncate(input.len() - 1);
            let vote_granted = match input.as_str() {
                "yes" => true,
                "no" => false,
                _ => {
                    println!("unrecognized input. Type 'yes' or 'no'.");
                    continue;
                }
            };
            return self.respond_to_vote(vote_granted);
        }
        // self.voted_for = Some(vote_for);
        // self.voted_for = Some(req.candidate_id);
    }

    async fn append_entries(
        &self,
        request: Request<AppendEntriesRequest>,
    ) -> Result<Response<AppendEntriesResponse>, Status> {
        let self = &mut self.lock().unwrap();
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
        return self.respond_to_ae(true);
    }
}

pub fn new_raft_server(address: &str, peers: Vec<String>) -> RaftServer<RaftNode> {
    return RaftServer::new(new_node(address, peers));
}
