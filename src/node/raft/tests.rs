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
        mismatch_index: None,
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
        mismatch_index: None,
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
        mismatch_index: None,
    };
    test_append_entries(&mut follower, req, expected_response);
    let expected_log = Vec::from([
        follower.log[0].clone(),
        noop_entry(1),
        noop_entry(1),
        noop_entry(1),
    ]);
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
        mismatch_index: None,
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

#[test]
fn test_update_commit_idx1() {
    // Leader should update its commit index to 1 because highest
    // majority index where logs are matching is 1
    // match_index = { "1": 1, "2": 1, "3": 1 }
    let mut l = new_node();
    l.peers = vec![String::from("1"), String::from("2"), String::from("3")];
    l.commit_index = 0;
    l.log.push(noop_entry(4));
    l.current_term = 4;
    l.match_index = HashMap::from([
        ("1".to_string(), 1),
        ("2".to_string(), 1),
        ("3".to_string(), 1),
    ]);
    l.update_commit_idx();
    assert_eq!(l.commit_index, 1);
}

#[test]
fn test_update_commit_idx2() {
    // should still work even without maj index being consistent
    // match_index = { "1": 6, "2": 2, "3": 3}
    let mut m = new_node();
    m.peers = vec![String::from("1"), String::from("2"), String::from("3")];
    m.commit_index = 0;
    m.log.push(noop_entry(4));
    m.log.push(noop_entry(4));
    m.current_term = 4;
    m.match_index = HashMap::from([
        ("1".to_string(), 6),
        ("2".to_string(), 2),
        ("3".to_string(), 3),
    ]);
    m.update_commit_idx();
    assert_eq!(m.commit_index, 2);
}

#[test]
fn test_update_commit_idx3() {
    // 1/2 of nodes is not the majority
    // match_index = { "1": 1, "2": 2, "3": 2, "4": 1}
    let mut l = new_node();
    l.peers = vec![String::from("1"), String::from("2"), String::from("3"), String::from("4")];
    l.commit_index = 0;
    l.log.push(noop_entry(4));
    l.log.push(noop_entry(4));
    l.current_term = 4;
    l.match_index = HashMap::from([
        ("1".to_string(), 1),
        ("2".to_string(), 2),
        ("3".to_string(), 2),
        ("3".to_string(), 1)
    ]);
    l.update_commit_idx();
    assert_eq!(l.commit_index, 1);
}

// also need tests for failed consistency and such
#[test]
fn test_commit_idx() {
    // leader node with 2 entries, follower node with 1
    // leader sends append entries, follower responds back
    // and leader should update commitindex from 1 to 2
    let mut l = new_node();
    l.state = State::Leader;
    l.log.push(LogEntry {
        command: None,
        term: 1,
    });

    // should break down send_AE into sections
    // creating the request
    // sending the request and getting a response
    // handling the response
    let responder1 = "follower1";
    let responder2 = "follower2";
    let r = AppendEntriesResponse {
        term: 1,
        success: true,
        mismatch_index: None,
    };
    l.handle_ae_response(responder1.to_string(), r.clone());
    l.handle_ae_response(responder2.to_string(), r.clone());
    l.update_commit_idx();

    // leader gets put request, calls replicate, which sends AE. send_AE doesn't return anything
    // but it does get messages from other nodes about whether AEs were successful
    // input: AEresponse
    // side effect/output: updated commit index, match index

    let expected_commit = 1;
    assert_eq!(l.commit_index, expected_commit);
}

#[tokio::test]
async fn test_follower() {
    let mut f = new_node();
    f.start_follower().await;
    assert_eq!(f.state, State::Candidate);
}
