use crate::node::raft_rpcs::LogEntry;

use super::*;

fn new_node() -> Node {
    let (_, rx) = mpsc::unbounded_channel();
    return Node::new(String::from("abc"), vec![], rx);
}

fn test_request_vote(receiver: &mut Node, req: VoteRequest, expected: VoteResponse) {
    match receiver.handle_request_vote(tonic::Request::new(req)) {
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
        last_log_index: 0,
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
        last_log_index: 0,
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
        last_log_index: 0,
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
        term: 2,
        last_log_index: 0,
        last_log_term: 0,
    };
    let expected_response = VoteResponse {
        term: 2,
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
        last_log_index: 0,
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
    follower.voted = true;
    let req = VoteRequest {
        candidate_id: String::from("candidate"),
        term: 1,
        last_log_index: 0,
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
    follower.voted = true;
    let req = VoteRequest {
        candidate_id: String::from("candidate"),
        term: 2,
        last_log_index: 0,
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
    match receiver.handle_append_entries(tonic::Request::new(req)) {
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
        prev_log_index: 0,
        prev_log_term: 0,
        entries: Vec::new(),
        leader_commit: 0,
    };
    let expected_response = AppendEntriesResponse {
        term: 1,
        success: true,
        mismatch_index: None
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
        prev_log_index: 0,
        prev_log_term: 0,
        entries: Vec::from([noop_entry(1)]),
        leader_commit: 0,
    };
    let expected_response = AppendEntriesResponse {
        term: 2,
        success: false,
        mismatch_index: None
    };
    test_append_entries(&mut follower, req, expected_response);
}

#[test]
fn test_ae_inconsistent() {
    // inconsistent because follower does not have log entry 1 (missing an entry)
    let mut follower = new_node();
    let req = AppendEntriesRequest {
        leader_id: String::from("leader"),
        term: 1,
        prev_log_index: 1,
        prev_log_term: 1,
        entries: Vec::from([noop_entry(1)]),
        leader_commit: 0,
    };
    let expected_response = AppendEntriesResponse {
        term: 1,
        success: false,
        mismatch_index: Some(1),
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
        prev_log_index: 0,
        prev_log_term: 0,
        entries: leader_entries,
        leader_commit: 0,
    };
    let expected_response = AppendEntriesResponse {
        term: 1,
        success: true,
        mismatch_index: None
    };
    test_append_entries(&mut follower, req, expected_response);
    let expected_log = Vec::from([follower.log[0].clone(), noop_entry(1), noop_entry(1), noop_entry(1)]);
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
        prev_log_index: 1,
        prev_log_term: 2,
        entries: leader_entries,
        leader_commit: 0,
    };
    let expected_response = AppendEntriesResponse {
        term: 4,
        success: true,
        mismatch_index: None
    };
    let leader_id = req.clone().leader_id;
    test_append_entries(&mut follower, req, expected_response);
    // for i in follower.log {
    //     println!("{}", i);
    // }
    
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
