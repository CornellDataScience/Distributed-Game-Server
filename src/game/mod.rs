// adapted https://github.com/not-fl3/macroquad/blob/master/examples/snake.rs
use std::collections::LinkedList;
use rand::Rng;

use crate::digs::Digs;
type coord = (i32,i32);

#[derive(PartialEq)]
pub enum Dir {
    Up,
    Down,
    Left,
    Right
}
pub struct Snake {
    pub head: coord,
    pub body: LinkedList<coord>,
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