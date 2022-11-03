use tonic::{transport::Server, Request, Response, Status};

use digs::leader_server::{Leader, LeaderServer};
use digs::{AppendEntriesReply, AppendEntriesRequest};

pub struct Node {}

impl Leader for Node {
    async fn append_entries(
        &self,
        request: Request<AppendEntriesRequest>, // Accept request of type HelloRequest
    ) -> Result<Response<AppendEntriesReply>, Status> {
        println!("Got a request: {:?}", request);

        let reply = digs::AppendEntriesReply {
            term,
            success
            // message: format!("Hello {}!", request.into_inner().name).into(), // We must use .into_inner() as the fields of gRPC requests and responses are private
        };
        
        // TODO: Do we also do actions to account for reply here?
        // e.g. revert to follower

        Ok(Response::new(reply)) // Send back our formatted greeting
    }
}

//runtime for the server. Allows it to respond to requests made to a port
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "[::1]:50051".parse()?;
    let node = Node::default();

    Server::builder()
        .add_service(LeaderServer::new(node))
        .serve(addr)
        .await?;

    Ok(())
}