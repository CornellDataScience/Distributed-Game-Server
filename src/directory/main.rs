#[macro_use] extern crate rocket;
use rocket::State;
use std::collections::HashSet;
use std::fs;
use std::sync::{Arc, Mutex};

// Stole code from https://github.com/SergioBenitez/Rocket/issues/478

struct PeersState {
    peers: HashSet<String>
}

type PeersStatePointer = Arc<Mutex<PeersState>>;

impl PeersState {
    fn new() -> PeersStatePointer {
        // let contents = fs::read_to_string("data/peers.txt")
        //     .expect("cannot read file");
        // let peers: HashSet<String> = contents
        //     .split("\n")
        //     .map(|addr| String::from(addr))
        //     .collect();
        let peers = HashSet::new();
        Arc::new(Mutex::new(PeersState { peers: peers }))
    }
}

#[get("/get-peers")]
fn get_peers(peers_state: &State<PeersStatePointer>) -> String {
    let mut peers_state = peers_state.lock().unwrap();
    let peers = &mut peers_state.peers;
    serde_json::to_string(&peers).unwrap()
}

#[get("/add-peer/<ip>")]
fn add_peer(ip: &str, peers_state: &State<PeersStatePointer>) -> String {
    let mut peers_state = peers_state.lock().unwrap();
    let peers = &mut peers_state.peers;
    peers.insert(String::from(ip));
    serde_json::to_string(&peers).unwrap()
}

#[get("/kick-peer?<ip>")]
fn kick_peer(ip: &str, peers_state: &State<PeersStatePointer>) -> String {
    let mut peers_state = peers_state.lock().unwrap();
    let peers = &mut peers_state.peers;
    peers.remove(&String::from(ip));
    serde_json::to_string(&peers).unwrap()
}

#[launch]
fn rocket() -> _ {
    rocket::build()
        .mount("/", routes![get_peers, kick_peer, add_peer])
        .manage(PeersState::new())
}