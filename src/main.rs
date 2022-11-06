use raft::{raft_client::RaftClient, VoteRequest};
use tonic::transport::{Channel, Server};
pub mod raft {
    tonic::include_proto!("raft");
}
use std::{env, net::SocketAddr, time::Duration};
mod node;

async fn start_rpc_server(addr: String) -> Result<(), Box<dyn std::error::Error>> {
    let socket_addr = match addr.parse::<SocketAddr>() {
        Ok(a) => a,
        Err(e) => {
            println!("could not parse address {}", addr);
            return Err(Box::new(e));
        }
    };
    println!("hosting raft RPC server at {}...", addr);
    Server::builder()
        .add_service(node::new_raft_server(addr.as_str(), Vec::new()))
        .serve(socket_addr)
        .await?;
    Ok(())
}

async fn connect_to(addr: String) -> Result<RaftClient<Channel>, Box<dyn std::error::Error>> {
    let node = RaftClient::connect(addr).await?;
    Ok(node)
}

async fn send_request_vote(
    node: Result<RaftClient<Channel>, Box<dyn std::error::Error>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut req = tonic::Request::new(VoteRequest {
        candidate_id: String::from("abc"),
        term: 1,
        last_log_index: None,
        last_log_term: 0,
    });
    req.set_timeout(Duration::from_secs(3600));
    println!("sending vote request to follower...");
    let mut node = match node {
        Ok(node) => node,
        Err(e) => {
            println!("unexpected error: {}", e);
            return Err(e);
        }
    };
    match node.request_vote(req).await {
        Err(e) => println!("unexpected error: {}", e),
        Ok(res) => println!("vote granted = {}", res.into_inner().vote_granted),
    }
    Ok(())
}

async fn get_my_ip() {
    if let Some(ip) = public_ip::addr().await {
        println!("public ip address: {:?}", ip);
    } else {
        println!("couldn't get an IP address");
    }
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 3 {
        println!("usage: cargo run -- <client/server> <ip>");
        return ();
    }

    get_my_ip().await;
    match String::from(&args[1]).as_str() {
        "client" => {
            let node = connect_to(String::from(&args[2])).await;
            match send_request_vote(node).await {
                Err(e) => println!("error starting server: {}", e),
                _ => (),
            }
        }
        "server" => match start_rpc_server(String::from(&args[2])).await {
            Err(e) => println!("error starting server: {}", e),
            _ => (),
        },
        r => println!("unknown role {}", r),
    };
}
