#[macro_use] extern crate rocket;
use rocket::State;
use rocket::serde::json::{Json,Value};
use rocket::serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::sync::{Arc, Mutex};
use digs::client::Client;

// Stole code from https://github.com/SergioBenitez/Rocket/issues/478

struct PeersState {
    peers: HashSet<String>
}

type PeersStatePointer = Arc<Mutex<PeersState>>;

impl PeersState {
    fn new() -> PeersStatePointer {
        let contents = fs::read_to_string("data/peers.txt")
            .expect("cannot read file");
        let peers: HashSet<String> = contents
            .split("\n")
            .map(|addr| String::from(addr))
            .collect();
        Arc::new(Mutex::new(PeersState { peers: peers }))
    }
}

#[derive(Debug,Serialize,Deserialize,FromForm)]
#[serde(crate = "rocket::serde")]
struct GameData {
    data:Vec<Vec<Vec<u32>>>
}


#[post("/put-info", format="json", data="<data>")]
fn put_info(peers_state: &State<PeersStatePointer>, data: Json<GameData>) {
    println!("{:?}", data);
    let peers_state = peers_state.lock().unwrap();
    let peers = &peers_state.peers;
    let p = peers.clone().into_iter().collect();
    let mut c = Client::new(p);
    // let key = String::from("key");
    // c.put(key.clone(), 1);

}

#[get("/get-peers")]
fn get_peers(peers_state: &State<PeersStatePointer>) -> String {
    let peers_state = peers_state.lock().unwrap();
    let peers = &peers_state.peers;
    serde_json::to_string(&peers).unwrap()
}

#[get("/get-peers/<ip>")]
fn get_peers_add_ip(ip: &str, peers_state: &State<PeersStatePointer>) -> String {
    let mut peers_state = peers_state.lock().unwrap();
    let peers = &mut peers_state.peers;
    peers.insert(String::from(ip));
    serde_json::to_string(&peers).unwrap()
}

#[get("/kick-peer/<ip>")]
fn kick_peer(ip: &str, peers_state: &State<PeersStatePointer>) -> String {
    let mut peers_state = peers_state.lock().unwrap();
    let peers = &mut peers_state.peers;
    peers.remove(&String::from(ip));
    serde_json::to_string(&peers).unwrap()
}

#[launch]
fn rocket() -> _ {
    rocket::build()
        .mount("/", routes![get_peers, kick_peer, get_peers_add_ip, put_info])
        .manage(PeersState::new())
}