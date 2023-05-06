
use macroquad::prelude::*;
use std::env;
use std::io::Read;
use digs::digs::Digs;
use digs::game::{Snake, Dir};

const SQUARES: i32 = 50;

#[macroquad::main("Snake")]
async fn main() {
    // gui/cli startup code
    // cargo r --bin game port dir_ip
    // directory hosted at http://localhost:8000/
    let args: Vec<String> = env::args().collect();
    let port = &args[1];
    let dir_ip = &args[2];
    let mut endgame = false;
    let mut snake = Snake::new();
    let mut t = get_time();
    // this might be important for fixing start times
    let mut digs = start_digs(port, dir_ip);
    loop {
        let mut res = reqwest::blocking::get(&format!("{}{}{}", dir_ip, "get-peers/",digs.id))
            .expect("Could not connect to directory server");
        let mut body = String::new();
        res.read_to_string(&mut body).unwrap();
        println!("{:?}", body);
    }
    loop {
        if !endgame {
            change_direction(&mut snake);
            if get_time() - t > 0.1 {
                t = get_time();
                move_snake(&mut snake);
                let serialized = serde_json::to_string(&snake).unwrap();
                digs.put(serialized).await;
                endgame = check_collision(&mut snake);
            }   
        } 
        draw_snake(&mut snake);
        next_frame().await;
    }
}

#[tokio::main]
async fn start_digs(port: &str, dir_ip: &str) -> Digs {
    let mut digs = Digs::new(&port, &dir_ip); // GUI code could go in here maybe?
    digs.register_node();
    digs.start().await;
    return digs;
}

fn check_collision(s: &mut Snake) -> bool {
    let check1 = s.head.0 < 0 || s.head.1 < 0 || s.head.0 >= SQUARES || s.head.1 >= SQUARES;
    let mut check2 = false;
    for (x,y) in &s.body {
        if x == &s.head.0 && y == &s.head.1 {
            check2 = true;
        }
    }
    return check1 || check2;
}

fn draw_snake(s: &mut Snake) {
    clear_background(BLACK);
    let game_size = screen_width().min(screen_height());
    let offset_x = (screen_width() - game_size) / 2. + 10.;
    let offset_y = (screen_height() - game_size) / 2. + 10.;
    let sq_size = (screen_height() - offset_y * 2.) / SQUARES as f32;
    draw_rectangle(offset_x, offset_y, game_size - 20., game_size - 20., WHITE);
    for i in 1..SQUARES {
        draw_line(
            offset_x,
            offset_y + sq_size * i as f32,
            screen_width() - offset_x,
            offset_y + sq_size * i as f32,
            2.,
            LIGHTGRAY,
        );
    }
    for i in 1..SQUARES {
        draw_line(
            offset_x + sq_size * i as f32,
            offset_y,
            offset_x + sq_size * i as f32,
            screen_height() - offset_y,
            2.,
            LIGHTGRAY,
        );
    }
    draw_rectangle(
        offset_x + s.head.0 as f32 * sq_size,
        offset_y + s.head.1 as f32 * sq_size,
        sq_size,
        sq_size,
        DARKGREEN,
    );
    for (x, y) in &s.body {
        draw_rectangle(
            offset_x + *x as f32 * sq_size,
            offset_y + *y as f32 * sq_size,
            sq_size,
            sq_size,
            LIME,
        );
    }
}

fn move_snake(s:&mut Snake) {
    s.body.push_front(s.head);
    s.body.pop_back();
    match s.dir {
        Dir::Up => s.head = (s.head.0, s.head.1-1),
        Dir::Down => s.head = (s.head.0, s.head.1+1),
        Dir::Left => s.head = (s.head.0-1, s.head.1),
        Dir::Right => s.head = (s.head.0+1, s.head.1)
    }
}

fn change_direction(s: &mut Snake) {
    if is_key_down(KeyCode::Up) && s.dir != Dir::Down {
        s.dir = Dir::Up;
    } else if is_key_down(KeyCode::Down) && s.dir != Dir::Up {
        s.dir = Dir::Down;
    } else if is_key_down(KeyCode::Right) && s.dir != Dir::Left {
        s.dir = Dir::Right;
    } else if is_key_down(KeyCode::Left) && s.dir != Dir::Right {
        s.dir = Dir::Left;
    }
}