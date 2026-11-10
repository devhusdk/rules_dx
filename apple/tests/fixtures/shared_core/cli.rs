use shared_tasks::{reply, Board, CORE_VERSION};

fn main() {
    let mut board = Board::load("").expect("empty board loads");
    println!("core={CORE_VERSION} {}", reply(&mut board, "add check"));
}
