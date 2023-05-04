use digs::client::Client;
use std::{io::Read, fs};

fn get_peers_from_file() -> Vec<String>{
    let contents = fs::read_to_string("../data/peers.txt").expect("cannot read file");
    let peers: Vec<String> = contents
        .split("\n")
        .filter(|addr| !addr.is_empty())
        .map(|addr| String::from("http://") + &String::from(addr))
        .collect();
    peers
}

#[tokio::main]
async fn main() {
    // set up a client and make 2 requests, a put then a get
    let mut res = reqwest::blocking::get("http://localhost:8000/get-peers")
        .expect("Could not connect to directory server");
    let mut body = String::new();
    res.read_to_string(&mut body).unwrap();
    let peers: Vec<String> = serde_json::from_str::<Vec<String>>(&body)
        .expect("Could not parse JSON response")
        .into_iter()
        .filter(|addr| !addr.is_empty())
        .map(|addr| String::from("http://") + &addr)
        .collect();
    let mut c = Client::new();
    c.set_peers(peers);
    let key = String::from("key");
    c.put(&r#"{ "key" : 1 }"#.to_string());
    println!("{}", c.get(key.clone()));
}
