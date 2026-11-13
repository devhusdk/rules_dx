#![deny(clippy::indexing_slicing, clippy::string_slice)]
//! Prints a data file resolved through the runfiles library.

use runfiles::{Runfiles, rlocation};

fn main() {
    let runfiles = Runfiles::create().expect("runfiles");
    let name = std::env::args().nth(1).expect("data path");
    let path = rlocation!(runfiles, name).expect("data file must resolve");
    print!("{}", std::fs::read_to_string(path).expect("data file must be readable"));
}
