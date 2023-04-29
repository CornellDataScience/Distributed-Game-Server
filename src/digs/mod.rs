use std::net::SocketAddr;

use crate::{
    client::Client,
    node::{raft::Node, rpc::{self, RaftRPCHandler}},
    util,
};

use tokio::sync::{mpsc};

pub struct Digs {
    id: String,
    socket: SocketAddr,
    peers: Vec<String>,
    // directory: Connection
    rpc_handler: RaftRPCHandler,
    client: Client,
    server: Node,
}
/// use case
/// if starting new server, will need to specify server ips and register to directory
/// if starting from existing server, join server through directory
/// May want a GUI/CLI which asks user whether they would like to start new server or join existing
/// 
impl Digs {
    /// start running Digs on port, with peers specified in peers
    pub fn new(port: &str, dir_ip: &str, game_name: &str) -> Self {
        let (id, socket) = util::makeSocket(port.to_string());
        let c = Client::new();
        // could try to combine rpc handler into node
        let (tx, rx) = mpsc::unbounded_channel();
        let n = Node::new(id, rx);
        let rpc_handler = rpc::RaftRPCHandler::new(tx);

        Self {
            id: id,
            socket: socket.unwrap(),
            peers: vec![],
            rpc_handler: rpc_handler,
            client: c,
            server: n,
        }
    }

    /// registers [game_name]->[peers] on directory server hosted at [dir_ip]
    pub fn register_server(&mut self, peers: Vec<String>, dir_ip: &str, game_name: &str) {
        // TODO: register peers to directory

        // make sure not to add itself in peers
        self.peers = peers;
        self.client.set_peers(peers);
        self.server.set_peers(peers);
    }

    /// joins an existing server through [dir_ip] and [game_name]
    pub fn join_server(&mut self){
        // get dir_ip[game_name]
        // update peers, client, and server
        // put dir_ip[game_name].append(self.id)
    }

    // TODO: tokio::spawn client vs server in different threads?
    // TODO: should there be a function that should be called to cleanup when server and such is shut down?
    pub async fn start(mut self) {
        println!("starting digs...");
        println!("connecting to directory...");

        println!("starting local server...");
        // start RPC server+raft server and client on different threads
        // main thread is the game which makes get/put reqs
        tokio::spawn(rpc::start_rpc_server(self.socket, self.rpc_handler));
        self.server.start().await;
        println!("client connecting to servers...");
        self.client.start();
        println!("digs started!");
    }

    pub async fn get(&mut self, key: String) {
        self.client.get(key);
    }

    pub async fn put(&mut self, key: String, value: String) {
        self.client.put(key, value);
    }
}
