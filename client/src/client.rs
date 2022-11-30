use std::collections::HashMap;

use futures::executor::block_on;
use rand::Rng;
use tonic::transport::Channel;
pub mod raft {
    tonic::include_proto!("raft");
}

use raft::raft_client::RaftClient;

use self::raft::GetRequest;

pub struct Client {
    peers: Vec<String>,
    connections: HashMap<String, RaftClient<Channel>>,
    current_leader: Option<String>,
}

impl Client {
    pub fn new(peers: Vec<String>) -> Self {
        let mut connections = HashMap::new();
        peers.clone().into_iter().for_each(|ip| {
            connections.insert(ip.clone(), block_on(RaftClient::connect(ip)).unwrap());
        });
        return Client {
            peers: peers,
            connections: connections,
            current_leader: None,
        };
    }

    pub fn get(&mut self, key: String) -> i64 {
        let dst_ip = match &self.current_leader {
            None => {
                let n = rand::thread_rng().gen_range(0..self.peers.len());
                &self.peers[n]
            }
            Some(l) => l,
        };
        let mut dst = self.connections.get(dst_ip).unwrap().clone();
        let req = GetRequest { key: key.clone() };
        match block_on(dst.get(req)) {
            Ok(res) => {
                let r = res.into_inner();
                if r.success {
                    return r.value;
                }
                self.current_leader = r.leader_id;
            }
            _ => (),
        }
        return self.get(key);
    }
}
