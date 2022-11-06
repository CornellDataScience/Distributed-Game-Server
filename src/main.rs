use raft::{raft_client::RaftClient, VoteRequest};
use tonic::transport::{Channel, Server};
pub mod raft {
    tonic::include_proto!("raft");
}
use std::fs;
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

async fn connect_to(addr: String) -> Option<RaftClient<Channel>> {
    match RaftClient::connect(addr).await {
        Err(_) => None,
        Ok(node) => Some(node),
    }
}

async fn send_request_vote(
    mut node: RaftClient<Channel>,
) -> Result<tonic::Response<raft::VoteResponse>, tonic::Status> {
    let mut req = tonic::Request::new(VoteRequest {
        candidate_id: String::from("abc"),
        term: 1,
        last_log_index: None,
        last_log_term: 0,
    });
    req.set_timeout(Duration::from_secs(3600));
    println!("sending vote request to follower...");
    return node.request_vote(req).await;
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
    let contents = fs::read_to_string("data/peers.txt").expect("cannot read file");
    let peers: Vec<&str> = contents.split(",").collect();
    let n = peers.len();
    if n == 0 {
        println!("please specify peers in data/peers.txt")
    }
    if args.len() < 2 {
        println!("usage: cargo run -- <client/server>");
        return ();
    }
    get_my_ip().await;
    match String::from(&args[1]).as_str() {
        "client" => {
            let mut votes = 0;
            for addr in peers {
                if let Some(node) = connect_to(String::from(addr)).await {
                    match send_request_vote(node).await {
                        Err(e) => println!("error starting server: {}", e),
                        Ok(response) => {
                            if response.into_inner().vote_granted {
                                votes += 1;
                            }
                        }
                    }
                }
                if votes >= n / 2 + 1 {
                    println!("majority voted received: {} out of {}", votes, n);
                    return;
                }
            }
            println!("only {} votes received: (need {})", votes, n);
        }
        "server" => match start_rpc_server(String::from("[::1]:8080")).await {
            Err(e) => println!("error starting server: {}", e),
            _ => (),
        },
        r => println!("unknown role {}", r),
    };
}
