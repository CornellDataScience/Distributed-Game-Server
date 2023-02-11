# Distributed-Game-Server

An implementation of the Raft Consensus algorithm for the Cornell Data Science Distributed Game Server project.

## Run Raft Node

- First, declare the list of peer IP addresses in data/peers.txt, one on each line.
- To run a Raft node at a certain port, run `cargo run --bin node <port>` from any directory.
- Type 'start' when prompted to start up the node and connect to its peers.

## Run Raft Client

- First, declare the list of peer IP addresses in data/peers.txt, one on each line.
- Run `cargo run --bin client <port>` from any directory.

## Tests

- To run the tests, run `cargo test` from any directory.


## Dependencies

protobuf3

### Mac

`brew install protobuf`

### Ubuntu

`apt-get protobuf-compiler` on Ubuntu may not get the right version. 
Install a precompiled binary of version 3.15.8+ from https://grpc.io/docs/protoc-installation/