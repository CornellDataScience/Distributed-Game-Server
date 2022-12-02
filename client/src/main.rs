mod client;
use client::Client;
use std::fs;
use clap::{Parser, Subcommand};
// #[tokio::main]
// async fn main() {
//     let contents = fs::read_to_string("data/peers.txt").expect("cannot read file");
//     let peers: Vec<String> = contents
//         .split("\n")
//         .map(|addr| String::from("http://") + &String::from(addr))
//         .collect();
//     let mut c = Client::new(peers);
//     let key = String::from("key");
//     c.put(key.clone(), 1);
//     println!("{}", c.get(key.clone()));
// }

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    // package_name: String
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    // Start {
    //     peers: Vec<String>
    // },
    /// get data from server
    Get {
        key: String
    },
    Put {
        key: String,
        value: i64
    }
}

fn main() {
    let cli = Cli::parse();
    // println!("{:?}", cli);
    // let mut client = None;
    let contents = fs::read_to_string("data/peers.txt").expect("cannot read file");
    let peers: Vec<String> = contents
        .split("\n")
        .map(|addr| String::from("http://") + &String::from(addr))
        .collect();
    let client = Some(Client::new(peers.to_vec()));

    match &cli.command {
        // Commands::Start { peers } => {
        //     client = Some(Client::new(peers.to_vec()));
        //     println!("Client created");
        // }
        Commands::Get { key } => {
            println!("Get");
            match client {
                Some(mut client) => {
                    client.get(key.to_string());
                }
                None => {
                    println!("Client does not exist yet. Please create one using [start]")
                }
            }
        }
        Commands::Put { key, value } => {
            println!("Put");
            match client {
                Some(mut client) => {
                    client.put(key.to_string(), value.clone());
                }
                None => {
                    println!("Client does not exist yet. Please create one using [start]")
                }
            }
        }
    }
}

