//! Consumer probe that only compiles under the consumer module's own `rust.toolchain` flags.

#[cfg(not(consumer_toolchain))]
compile_error!("consumer_report needs --cfg=consumer_toolchain from the consumer rust.toolchain extra_rustc_flags");

use shared_tasks::{reply, Board, CORE_VERSION};

/// Board the probe loads; fixed so the output is exact.
const BOARD: &str = "[x] Pin the Rust toolchain\n[ ] Override the toolchain\n";

fn main() {
    let mut board = Board::load(BOARD).expect("the fixed board parses");
    println!("{}", reply(&mut board, "done 2"));
    println!("summary {}", board.summary());
    println!("core={CORE_VERSION} toolchain=consumer");
}
