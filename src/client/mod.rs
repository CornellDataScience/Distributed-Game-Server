use std::collections::HashMap;

use futures::executor::block_on;
use rand::Rng;
use tonic::transport::Channel;

use self::raft_rpc::{raft_rpc_client::RaftRpcClient, GetRequest, PutRequest};

pub mod raft_rpc {
    tonic::include_proto!("raftrpc");
}

pub struct Client {
    peers: Vec<String>,
    connections: HashMap<String, RaftRpcClient<Channel>>,
    current_leader: Option<String>,
}

impl Client {
    /// Creates a new client from a vector of server nodes
    pub fn new(peers: Vec<String>) -> Self {
        let mut connections = HashMap::new();
        peers.clone().into_iter().for_each(|ip| {
            connections.insert(ip.clone(), block_on(RaftRpcClient::connect(ip)).unwrap());
        });
        return Client {
            peers: peers,
            connections: connections,
            current_leader: None,
        };
    }
    /// If current leader unknown, finds the leader of the server by sending a
    /// message to a random server in the cluster
    fn find_leader(&mut self) -> RaftRpcClient<Channel> {
        let dst_ip = &match &self.current_leader {
            None => {
                let n = rand::thread_rng().gen_range(0..self.peers.len());
                self.peers[n].clone()
            }
            Some(l) => String::from("http://") + l,
        };
        self.connections.get(dst_ip).unwrap().clone()
    }

    /// Gets the value of a key from the leader
    pub fn get(&mut self, key: String) -> i64 {
        let mut dst = self.find_leader();
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
        self.get(key)
    }

    /// Pushes a key value pair to the leader
    pub fn put(&mut self, key: String, value: i64) -> bool {
        let mut dst = self.find_leader();
        let req = PutRequest {
            key: key.clone(),
            value: value,
            serial_number: 0,
        };
        match block_on(dst.put(req)) {
            Ok(res) => {
                let r = res.into_inner();
                if r.success {
                    return true;
                }
                self.current_leader = r.leader_id;
            }
            _ => (),
        }
        self.put(key, value)
    }
}
