use digs::client::Client;
use std::fs;
use std::io::Read;
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
    let mut c = Client::new(peers);
    let key = String::from("key");
    c.put(key.clone(), 1);
    println!("{}", c.get(key.clone()));
}
