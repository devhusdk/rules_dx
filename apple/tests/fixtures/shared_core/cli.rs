use shared_tasks::{Board, CORE_VERSION, reply};

fn main() {
    let mut board = Board::load("").expect("empty board loads");
    println!("core={CORE_VERSION} {}", reply(&mut board, "add check"));
}
