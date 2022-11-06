use raft::{raft_client::RaftClient, VoteRequest};
use tonic::transport::Server;
pub mod raft {
    tonic::include_proto!("raft");
}
use std::env;
mod node;

async fn start_rpc_server(addr: String) -> Result<(), Box<dyn std::error::Error>> {
    println!("hosting raft RPC server at {}...", addr);
    Server::builder()
        .add_service(node::new_raft_server(addr.as_str(), Vec::new()))
        .serve(addr.parse().unwrap())
        .await?;
    Ok(())
}

async fn connect_to(addr: String) -> Result<(), Box<dyn std::error::Error>> {
    let mut follower = RaftClient::connect(addr).await?;
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
        println!("specify 'client' or 'server' role in command-line argument");
        return ();
    }
    println!("{}", String::from(&args[2]));
    return match String::from(&args[1]).as_str() {
        "client" => match connect_to(String::from(&args[2])).await {
            Err(e) => println!("error starting server: {}", e.to_string()),
            Ok(_) => (),
        },
        "server" => match start_rpc_server(String::from(&args[2])).await {
            Err(e) => println!("error starting server: {}", e.to_string()),
            Ok(_) => (),
        },
        r => println!("unknown role {}", r),
    };
}
