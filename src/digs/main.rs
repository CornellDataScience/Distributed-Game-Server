use std::env;

use digs::digs::Digs;

// there may be some weird things with tokio async/sync
#[tokio::main]
async fn main() {
    // gui/cli startup code
    // cargo r --bin digs port dir_ip
    // directory hosted at http://localhost:8000/
    let args: Vec<String> = env::args().collect();
    let port = &args[2];
    let dir_ip = &args[3];

    let mut digs = Digs::new(&port, &dir_ip); // GUI code could go in here maybe?

    digs.register_node();
    digs.start().await;

    // game code
    // digs.put()
    // digs.get()
}
