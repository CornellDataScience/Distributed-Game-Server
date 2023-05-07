
use std::{collections::HashMap, io::Read};

use futures::executor::block_on;
use rand::Rng;
use tonic::transport::Channel;

use self::raft_rpc::{raft_rpc_client::RaftRpcClient, GetRequest, PutRequest};


pub mod raft_rpc {
    tonic::include_proto!("raftrpc");
}

#[derive(Debug, Clone)]
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
        // println!("finding leader");
        let dst_ip = &match &self.current_leader {
            None => {
                let mut res = reqwest::blocking::get("http://localhost:8000/get-leader/")
                .expect("Could not connect to directory server");
                let mut body = String::new();
                res.read_to_string(&mut body).unwrap();
                if body != "none" {
                    "http://".to_string() + &body
                } else {
                    let n = rand::thread_rng().gen_range(0..self.peers.len());
                    self.peers[n].clone()
                }
            }
            Some(l) => "http://".to_string() + l,
        };
        self.current_leader = Some(dst_ip.to_string());
        self.connections.get(dst_ip).unwrap().clone()
    }

    /// Gets the value of a key from the leader
    pub fn get(&mut self, key: String) -> String {
        let dst = self.find_leader();
        // println!("getting {:?}", self.current_leader);

        let req = GetRequest { key: key.clone() };
        match block_on(dst.clone().get(req)) {
            Ok(res) => {
                // println!("{:?}",res);
                let r = res.into_inner();
                self.current_leader = r.leader_id;
                if r.success {
                    // println!("{:?}", r.value);
                    return r.value;
                } else {
                    // TODO: FIX
                    return 0.to_string();
                }
            }
            _ => {
                println!("uhoh");
                return 0.to_string();
            }
        }
        // self.get(key)
    }

    /// Pushes a key value pair to the leader
    pub fn put(&mut self, key: String, value: String) -> Result<(), String> {
        let mut dst = self.find_leader();
        // println!("putting {:?}", self.current_leader);

        let req = PutRequest {
            key: key.clone(),
            data: value.to_string(),
            serial_number: 0,
        };
        match block_on(dst.put(req)) {
            Ok(res) => {
                // println!("{:?}", res);
                let r = res.into_inner();
                self.current_leader = r.leader_id.clone();
                // try finding the leader again if put unsuccessful
                if !r.success {
                    return self.put(key, value);
                }
                let url = format!("http://localhost:8000/set-leader/{ldr}", ldr=r.leader_id.clone().unwrap());
                reqwest::blocking::get(&url);
                return Ok(())
            }
            // Expected Ok, but failed for some reason
            _ => return Err("Put request was not Ok for some reason".to_string()),
        }
        // self.put(key, value)
    }
}