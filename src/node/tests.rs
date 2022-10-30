use super::*;
use futures::executor::block_on;

fn test_request_vote(receiver: RaftNode, req: VoteRequest, expected: VoteResponse) {
    match block_on(receiver.request_vote(tonic::Request::new(req))) {
        Err(e) => println!("unexpected request_vote error: {}", e),
        Ok(res) => {
            assert_eq!(res.into_inner(), expected);
        }
    }
}

#[test]
fn test_vote_granted() {
    let follower = new_node("follower", Vec::new());
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
    let f_cpy = Arc::clone(&follower);
    test_request_vote(follower, req, expected_response);
    assert_eq!(
        f_cpy.lock().unwrap().voted_for,
        Some(String::from("candidate"))
    );
}
#[test]
fn test_candidate_older_term() {
    let follower = new_node("follower", Vec::new());
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
    test_request_vote(follower, req, expected_response);
}

#[test]
fn test_candidate_log_old_same_term() {
    let follower = new_node("follower", Vec::new());
    {
        follower.lock().unwrap().log.push(LogEntry {
            command: None,
            term: 1,
        });
    }
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
    test_request_vote(follower, req, expected_response);
}

#[test]
fn test_candidate_log_old_newer_term() {
    let follower = new_node("follower", Vec::new());
    {
        follower.lock().unwrap().log.push(LogEntry {
            command: None,
            term: 1,
        });
    }
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
    test_request_vote(follower, req, expected_response);
}

#[test]
fn test_candidate_newer_term() {
    let follower = new_node("follower", Vec::new());
    {
        follower.lock().unwrap().state = State::Leader;
    }
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
    let f_cpy = Arc::clone(&follower);
    test_request_vote(follower, req, expected_response);
    assert_eq!(f_cpy.lock().unwrap().state, State::Follower);
    assert_eq!(
        f_cpy.lock().unwrap().voted_for,
        Some(String::from("candidate"))
    );
}

#[test]
fn test_already_voted() {
    let follower = new_node("follower", Vec::new());
    {
        follower.lock().unwrap().voted_for = Some(String::from("other candidate"));
    }
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
    test_request_vote(follower, req, expected_response);
}

#[test]
fn test_leader_replaced() {
    let follower = new_node("follower", Vec::new());
    follower.lock().unwrap().voted_for = Some(String::from("other candidate"));
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
    let f_cpy = Arc::clone(&follower);
    test_request_vote(follower, req, expected_response);
    assert_eq!(
        f_cpy.lock().unwrap().voted_for,
        Some(String::from("candidate"))
    );
}

fn test_append_entries(
    receiver: RaftNode,
    req: AppendEntriesRequest,
    expected: AppendEntriesResponse,
) {
    match block_on(receiver.append_entries(tonic::Request::new(req))) {
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
    let follower = new_node("follower", Vec::new());
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
    test_append_entries(follower, req, expected_response);
}

#[test]
fn test_ae_older_term() {
    let follower = new_node("follower", Vec::new());
    follower.lock().unwrap().current_term = 2;
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
    test_append_entries(follower, req, expected_response);
}

#[test]
fn test_ae_inconsistent() {
    let follower = new_node("follower", Vec::new());
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
    test_append_entries(follower, req, expected_response);
}

#[test]
fn test_ae_initial() {
    let follower = new_node("follower", Vec::new());
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
    let f_cpy = Arc::clone(&follower);
    test_append_entries(follower, req, expected_response);
    let expected_log = Vec::from([noop_entry(1), noop_entry(1), noop_entry(1)]);
    assert_eq!(f_cpy.lock().unwrap().log, expected_log);
}

#[test]
fn test_ae_overwrite() {
    let follower = new_node("follower", Vec::new());
    let follower_log = Vec::from([noop_entry(1), noop_entry(2), noop_entry(3)]);
    follower.lock().unwrap().log = follower_log;
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
    let f_cpy = Arc::clone(&follower);
    let leader_id = req.clone().leader_id;
    test_append_entries(follower, req, expected_response);
    let expected_log = Vec::from([
        noop_entry(1),
        noop_entry(2),
        noop_entry(4),
        noop_entry(4),
        noop_entry(4),
    ]);
    let f = f_cpy.lock().unwrap();
    assert_eq!(f.log, expected_log);
    assert_eq!(f.voted_for, Some(leader_id));
}
