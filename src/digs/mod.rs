use std::{net::SocketAddr, io::Read};

use crate::{
    client::Client,
    node::{raft::Node, rpc::{self, RaftRPCHandler}},
    util,
};

use tokio::sync::{mpsc};

pub struct Digs {
    pub id: String,
    dir_ip: String,
    client: Client,
    server: Node,
}
/// use case
/// if starting new server, will need to specify server ips and register to directory
/// if starting from existing server, join server through directory
/// May want a GUI/CLI which asks user whether they would like to start new server or join existing
impl Digs {
    pub fn new(port: &str, dir_ip: &str) -> Self {
        let (id, socket) = util::makeSocket(port.to_string());
        let c = Client::new();
        // could try to combine rpc handler into node
        let (tx, rx) = mpsc::unbounded_channel();
        let n = Node::new(id.clone(), rx);
        let rpc_handler = rpc::RaftRPCHandler::new(tx);
        tokio::spawn(rpc::start_rpc_server(socket.unwrap(), rpc_handler));

        Self {
            id: id,
            dir_ip: dir_ip.to_string(),
            client: c,
            server: n,
        }
    }

    // TODO: Flow for if cluster config were implemented
    // for an individual machine,
    // get node list through [dir_ip]
    // if cluster didn't exist yet, will need to register node on directory server and start running client+server code
    // otherwise
    // start up a client to make a cluster config change request
    // once accepted, add server node to the cluster by registering node on directory server hosted at [dir_ip]
    // and running server code
    /// unused for now
    // pub fn register_node(&mut self, peers: Vec<String>, dir_ip: &str) {
    //     // make sure not to add itself in peers
    //     self.peers = peers.clone();
    //     self.client.set_peers(peers.clone());
    //     self.server.set_peers(peers);
    // }
    // pub fn join_server() {
    // }

    // Flow for showcase:
    // start up rpc servers so can accept connections
    // register self on directory
    // wait until all participating nodes have registered with the directory
    // get peers from directory and start client and server
    pub fn register_node(&mut self){
        let mut add_res = reqwest::blocking::get(&format!("{}{}{}", self.dir_ip, "get-peers/", self.id))
            .expect("Could not connect to directory server");
        let mut body = String::new();
        add_res.read_to_string(&mut body).unwrap();
        println!("Result from adding own id {}", body);
        // wait for all rpc servers to start - implement on game side
    }

    // TODO: tokio::spawn client vs server in different threads?
    // TODO: should there be a function that should be called to cleanup when server and such is shut down?
    pub async fn start(&mut self) {
        println!("starting digs...");
        println!("connecting to directory...");
        let mut res = reqwest::blocking::get(&format!("{}get-peers/", self.dir_ip))
            .expect("Could not connect to directory server");
        let mut body = String::new();
        res.read_to_string(&mut body).unwrap();
        let peers: Vec<String> = serde_json::from_str::<Vec<String>>(&body)
            .expect("Could not parse JSON response")
            .into_iter()
            .filter(|addr| !addr.is_empty())
            .map(|addr| String::from("http://") + &addr)
            .collect();
            println!("starting local server...");
        self.server.set_peers(peers.clone());
        self.client.set_peers(peers);
        // may want to start raft server and client on different threads using the tokio scheduler
        // i think this will also resolve issues that may arise with start() being async
        // but how to implement Send?
        // main thread is the game which makes get/put reqs
        self.server.start();
        println!("client connecting to servers...");
        self.client.start();
        println!("digs started!");
    }

    pub async fn get(&mut self, key: String) {
        self.client.get(key);
    }

    pub async fn put(&mut self, data: String) {
        self.client.put(&data);
    }
}
