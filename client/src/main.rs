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
    let key = String::from("key");
    c.put(key.clone(), 1);
    println!("{}", c.get(key.clone()));
}
