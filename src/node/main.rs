use local_ip_address::local_ip;
use digs::node::{raft, rpc};
use std::{env, fs, io, net::SocketAddr};
use tokio::sync::mpsc;

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();

    println!("number of args passed:{} \n {:#?}", args.len(), args);
    // command format: cargo run <--bin node> <optional port>
    // run servers from localhost at some port (default 8080)
    let mut port = String::from(":8080");
    if args.len() >= 2 {
        port = ":".to_string() + &String::from(&args[1]);
    }
    // server_addr is <ip><:port>
    // by default use local ip, if localhost is specified, use [::1]
    let mut server_addr = local_ip().unwrap().to_string() + &port;
    if args.len() == 3 && args[2] == "localhost" {
        server_addr = String::from("[::1]") + &port;
    }
    println!("{:?}",server_addr.parse::<SocketAddr>());
    let socket = match server_addr.parse::<SocketAddr>() {
        Err(e) => {
            println!("could not parse IP {server_addr}: {e}");
            return;
        }
        Ok(a) => a,
    };

    // read from peers.txt to get the ip addresses of the other server nodes, ignoring its own address
    // assumes user is running from src
    let contents = fs::read_to_string("../data/peers.txt").expect("cannot read file");
    let peers: Vec<String> = contents
        .split("\n")
        .filter(|addr| !addr.is_empty() && addr != &server_addr)
        .map(|addr| String::from("http://") + &String::from(addr))
        .collect();
    if peers.len() == 0 {
        println!("peers.txt must have at least one peer");
        return;
    }

    // RPC handler can forward RPCs to node via this channel
    let (tx, rx) = mpsc::unbounded_channel();
    let rpc_handler = rpc::RaftRPCHandler::new(tx);

    // start RPC server in new thread
    tokio::spawn(rpc::start_rpc_server(socket, rpc_handler));
    loop {
        // for now, since we need to wait for all other RPC servers
        // to start before calling node.start(), we will have it triggered
        // via stdin when a user types 'start'.
        println!("type 'start' to start the node");
        let mut input = String::new();
        let stdin = io::stdin();
        stdin.read_line(&mut input).unwrap();
        input.truncate(input.len() - 1);
        match input.as_str() {
            "start" => break,
            _ => continue,
        };
    }
    let mut node = raft::Node::new(server_addr, peers, rx);
    node.start().await;
}