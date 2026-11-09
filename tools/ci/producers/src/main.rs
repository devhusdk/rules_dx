//! `producers_bin` prints the compiler signatures of each file argument as a JSON object.

use dx_binfmt::producers;
use std::collections::BTreeMap;

fn main() {
    let mut found = BTreeMap::new();
    for path in std::env::args().skip(1) {
        let producers = std::fs::read(&path)
            .map(|bytes| producers(&bytes))
            .unwrap_or_default();
        found.insert(path, producers);
    }
    println!(
        "{}",
        serde_json::to_string(&found).expect("serialize producers")
    );
}
