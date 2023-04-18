use digs::client::Client;
use std::fs;
#[tokio::main]
async fn main() {
    // set up a client and make 2 requests, a put then a get
    let contents = fs::read_to_string("../data/peers.txt").expect("cannot read file");
    let peers: Vec<String> = contents
        .split("\n")
        .filter(|addr| !addr.is_empty())
        .map(|addr| String::from("http://") + &String::from(addr))
        .collect();
    let mut c = Client::new(peers);
    let key = String::from("key");
    c.put(key.clone(), 1);
    println!("{}", c.get(key.clone()));
}
