mod client;
use client::Client;
use std::fs;
#[tokio::main]
async fn main() {
    let contents = fs::read_to_string("data/peers.txt").expect("cannot read file");
    let peers: Vec<String> = contents
        .split("\n")
        .map(|addr| String::from("http://") + &String::from(addr))
        .collect();
    let mut c = Client::new(peers);
    println!("{}", c.get(String::from("key")));
}
