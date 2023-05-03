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
    let key1 = String::from("key1");
    let key2 = String::from("key2");
    
    c.put(key1.clone(), 1).await;
    c.put(key2.clone(), 2).await;
    // tokio::join!(
    //     c.get(key1.clone()),
    //     c.get(key2.clone()),
    // );
    println!("{:?}", c.get(key1.clone()).await);
    println!("{:?}", c.get(key2.clone()).await);
}
