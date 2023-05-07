#[macro_use] extern crate rocket;
use rocket::State;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};

// Stole code from https://github.com/SergioBenitez/Rocket/issues/478

struct PeersState {
    peers: HashSet<String>,
    leader: Option<String>
}
type PeersStatePointer = Arc<Mutex<PeersState>>;

impl PeersState {
    fn new() -> PeersStatePointer {
        let peers = HashSet::new();
        Arc::new(Mutex::new(PeersState { peers: peers, leader: None }))
    }
}

struct GameIdsState {
    game_ids: HashSet<String>
}
type GameIdsStatePointer = Arc<Mutex<GameIdsState>>;

impl GameIdsState {
    fn new() -> GameIdsStatePointer {
        let game_ids = HashSet::new();
        Arc::new(Mutex::new(GameIdsState { game_ids: game_ids }))
    }
}

#[get("/get-leader")]
fn get_leader(peers_state: &State<PeersStatePointer>) -> String {
    let peers_state = peers_state.lock().unwrap();
    let leader = &peers_state.leader;
    match leader {
        Some(s) => {
            return s.to_string();
        }, 
        None => {
            return "none".to_string();
        }
    }
}

#[get("/set-leader/<leader>")]
fn set_leader(leader: &str, peers_state: &State<PeersStatePointer>) -> String {
    let mut peers_state = peers_state.lock().unwrap();
    peers_state.leader = Some(String::from(leader));
    serde_json::to_string(&peers_state.leader).unwrap()
}


#[get("/get-games")]
fn get_game_ids(game_ids_state: &State<GameIdsStatePointer>) -> String {
    let game_ids_state = game_ids_state.lock().unwrap();
    let game_ids = &game_ids_state.game_ids;
    serde_json::to_string(&game_ids).unwrap()
}

#[get("/add-game/<id>")]
fn add_game_id(id: &str, game_ids_state: &State<GameIdsStatePointer>) -> String {
    let mut game_ids_state = game_ids_state.lock().unwrap();
    let game_ids = &mut game_ids_state.game_ids;
    game_ids.insert(String::from(id));
    serde_json::to_string(&game_ids).unwrap()
}

#[get("/get-peers")]
fn get_peers(peers_state: &State<PeersStatePointer>) -> String {
    let peers_state = peers_state.lock().unwrap();
    let peers = &peers_state.peers;
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
        .mount("/", routes![get_peers, kick_peer, add_peer, get_game_ids, add_game_id, get_leader, set_leader])
        .manage(PeersState::new())
        .manage(GameIdsState::new())
}