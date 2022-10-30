use raft::{raft_client::RaftClient, VoteRequest};
use tonic::transport::Server;
pub mod raft {
    tonic::include_proto!("raft");
}
use std::env;
mod node;

async fn start_rpc_server() -> Result<(), Box<dyn std::error::Error>> {
    let address = "[::1]:8080";
    println!("hosting raft RPC server at {}...", address);
    Server::builder()
        .add_service(node::new_raft_server(address, Vec::new()))
        .serve(address.parse().unwrap())
        .await?;
    Ok(())
}

async fn start_client() -> Result<(), Box<dyn std::error::Error>> {
    let mut follower = RaftClient::connect("http://[::1]:8080").await?;
    let req = VoteRequest {
        candidate_id: String::from("abc"),
        term: 1,
        last_log_index: None,
        last_log_term: 0,
    };
    println!("sending vote request to follower:");
    dbg!(req.clone());
    match follower.request_vote(tonic::Request::new(req)).await {
        Err(e) => println!("unexpected error: {}", e),
        Ok(res) => println!("vote granted = {}", res.into_inner().vote_granted),
    }
    Ok(())
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        println!("specify 'client' or 'server' role in command-line argument");
        return ();
    }
    return match String::from(&args[1]).as_str() {
        "client" => assert!(start_client().await.is_ok()),
        "server" => assert!(start_rpc_server().await.is_ok()),
        r => println!("unknown role {}", r),
    };
}
