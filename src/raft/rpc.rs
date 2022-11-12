use crate::raft::node::Event;
use crate::raft::raft::{
    raft_server::Raft, raft_server::RaftServer, AppendEntriesRequest, AppendEntriesResponse,
    VoteRequest, VoteResponse,
};
use std::error::Error;
use std::net::SocketAddr;
use tokio::sync::{mpsc, oneshot};
use tonic::{transport::Server, Code, Request, Response, Status};

pub struct RaftRPCHandler {
    // used for forwarding RPCs to main thread
    sender: mpsc::UnboundedSender<Event>,
}

impl RaftRPCHandler {
    pub fn new(sender: mpsc::UnboundedSender<Event>) -> Self {
        Self { sender: sender }
    }
}

fn internal_err(e: &dyn Error) -> Status {
    return Status::new(Code::Internal, format!("{}", e));
}

#[tonic::async_trait]
impl Raft for RaftRPCHandler {
    async fn request_vote(
        &self,
        request: Request<VoteRequest>,
    ) -> Result<Response<VoteResponse>, Status> {
        // simply forward RPC via the sender, and block
        // until response comes back.
        let (tx, rx) = oneshot::channel();
        self.sender
            .send(Event::RequestVote {
                req: request.into_inner(),
                tx: tx,
            })
            .map_err(|e| internal_err(&e))?;
        rx.await.map_err(|e| internal_err(&e))?
    }

    async fn append_entries(
        &self,
        request: Request<AppendEntriesRequest>,
    ) -> Result<Response<AppendEntriesResponse>, Status> {
        let (tx, rx) = oneshot::channel();
        self.sender
            .send(Event::AppendEntries {
                req: request.into_inner(),
                tx: tx,
            })
            .map_err(|e| internal_err(&e))?;
        rx.await.map_err(|e| internal_err(&e))?
    }
}

pub async fn start_rpc_server(
    addr: SocketAddr,
    handler: RaftRPCHandler,
) -> Result<(), tonic::transport::Error> {
    println!("hosting raft RPC server at {}...", addr);
    return Server::builder()
        .add_service(RaftServer::new(handler))
        .serve(addr)
        .await;
}
