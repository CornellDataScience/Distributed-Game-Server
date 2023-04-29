use digs::digs::Digs;

fn main() {
    // gui/cli startup code
    // cargo r --bin digs --new dir_ip game_name

    let mut digs = Digs::new("", "", ""); // GUI code could go in here maybe?
    match make_new {
        true => digs.register_server(peers, dir_ip, game_name),
        false => digs.join_server(dir_ip, game_name),
    }
    digs.start();

    // game code
    // digs.put()
    // digs.get()
}
