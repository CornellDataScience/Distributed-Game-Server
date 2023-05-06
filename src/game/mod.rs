// adapted https://github.com/not-fl3/macroquad/blob/master/examples/snake.rs
use std::collections::LinkedList;
use serde::{Serialize, Deserialize};
use rand::Rng;
type Coord = (i32,i32);

#[derive(PartialEq, Serialize, Deserialize)]
pub enum Dir {
    Up,
    Down,
    Left,
    Right
}

#[derive(Serialize, Deserialize)]
pub struct Snake {
    pub head: Coord,
    pub body: LinkedList<Coord>,
    pub dir: Dir,
}
impl Snake {
    pub fn new() -> Self {
        let mut x = rand::thread_rng().gen_range(30..35);
        let y = rand::thread_rng().gen_range(5..45);
        let h = (x, y);
        let mut b = LinkedList::new();
        for _ in 0..10 {
            x = x-1;
            b.push_back((x, y));
        }
        Self {
            head : h,
            body : b,
            dir : Dir::Up,
        }
    }
}