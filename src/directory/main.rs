#[macro_use] extern crate rocket;
use rocket::State;
use std::sync::{Arc, Mutex};
use std::collections::HashSet;

// Stole code from https://github.com/SergioBenitez/Rocket/issues/478

struct PeersState {
    peers: HashSet<String>
}

type PeersStatePointer = Arc<Mutex<PeersState>>;

impl PeersState {
    fn new() -> PeersStatePointer {
        Arc::new(Mutex::new(PeersState { peers: HashSet::new() }))
    }
}

#[get("/")]
fn index() -> &'static str {
    "Hello, world!"
}

#[get("/get-peers/<ip>")]
fn get_peers(ip: &str, peers_state: &State<PeersStatePointer>) -> String {
    // will this crash if the lock is in use? how to make it wait until lock is available again?
    // read chapter 16 of Rust book
    let mut peers_state = peers_state.lock().unwrap();
    let peers = &mut peers_state.peers;
    peers.insert(String::from(ip));
    format!("Added a peer {}. Peers: {:?}", ip, peers)
}

#[get("/kick-peer/<ip>")]
fn kick_peer(ip: &str, peers_state: &State<PeersStatePointer>) -> String {
    let mut peers_state = peers_state.lock().unwrap();
    let peers = &mut peers_state.peers;
    peers.remove(&String::from(ip));
    format!("Removed a peer {}. Peers: {:?}", ip, peers)
}

#[launch]
fn rocket() -> _ {
    rocket::build()
        .mount("/", routes![index, get_peers, kick_peer])
        .manage(PeersState::new())
}