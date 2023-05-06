use std::collections::HashMap;

use futures::executor::block_on;
use rand::Rng;
use tonic::transport::Channel;

use self::raft_rpc::{raft_rpc_client::RaftRpcClient, GetRequest, PutRequest};

use async_recursion::async_recursion;

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
            let future = block_on(RaftRpcClient::connect(ip.clone()));
            connections.insert(ip.clone(), future.unwrap());
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
        println!("finding leader");
        let dst_ip = &match &self.current_leader {
            None => {
                let n = rand::thread_rng().gen_range(0..self.peers.len());
                self.peers[n].clone()
            }
            Some(l) => String::from("http://") + l,
        };
        self.current_leader = Some(dst_ip.to_string());
        self.connections.get(dst_ip).unwrap().clone()
    }

    /// Gets the value of a key from the leader
    pub async fn get(&mut self, key: String) -> i64 {
        let mut dst = self.find_leader();
        let req = GetRequest { key: key.clone() };
        match dst.get(req).await {
            Ok(res) => {
                let r = res.into_inner();
                self.current_leader = r.leader_id;
                if r.success {
                    return r.value;
                } else {
                    // TODO: FIX
                    return 0;
                }
            }
            _ => return 0,
        }
        // self.get(key)
    }

    /// Pushes a key value pair to the leader
    #[async_recursion]
    pub async fn put(&mut self, key: String, value: i64) -> Result<(), String> {
        let mut dst = self.find_leader();
        println!("putting {:?}", self.current_leader);

        let req = PutRequest {
            key: key.clone(),
            value: value,
            serial_number: 0,
        };
        match dst.put(req).await {
            Ok(res) => {
                let r = res.into_inner();
                self.current_leader = r.leader_id;
                // try finding the leader again if put unsuccessful
                if !r.success {
                    return self.put(key, value).await;
                }
                return Ok(())
            }
            // Expected Ok, but failed for some reason
            _ => return Err("Put request was not Ok for some reason".to_string()),
        }
        // self.put(key, value)
    }
}
