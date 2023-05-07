use std::net::{SocketAddr, AddrParseError};

use local_ip_address::local_ip;

pub fn makeSocket(port: String) -> (String, Result<SocketAddr, AddrParseError>) {
    // server_addr is <ip><:port>
    // by default use local ip, if localhost is specified, use [::1]
    let mut server_addr = format!("{}:{}", local_ip().unwrap().to_string(), &port);
    // TODO: check if works without. can you access yourself using your own local ip? or do you have to use localhost
    // if args.len() == 3 && args[2] == "localhost" {
    //     server_addr = String::from("[::1]") + &port;
    // }
    println!("{:?}", server_addr.parse::<SocketAddr>());
    match &server_addr.parse::<SocketAddr>() {
        err @ Err(e) => {
            println!("could not parse IP {server_addr}: {e}");
            (server_addr, err.clone())
        }
        addr @ Ok(a) => (server_addr, addr.clone()),
    }
}
