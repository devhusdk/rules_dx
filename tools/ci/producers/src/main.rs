//! `producers_bin` prints the compiler signatures of each file argument as a JSON object.
//! `producers_bin --evidence OUT PLATFORM FACTS ROW_LABELS` appends the compiler evidence lines to OUT.
//! `producers_bin --lock-merge LOCKS_DIR OUT` merges the per-platform module locks into OUT.

#![deny(clippy::indexing_slicing, clippy::string_slice)]

mod evidence;
mod lock_merge;

use dx_binfmt::producers;
use std::collections::BTreeMap;

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(("--evidence", rest)) = args.split_first().map(|(mode, rest)| (mode.as_str(), rest))
    {
        return evidence::append(rest);
    }
    if let Some(("--lock-merge", rest)) = args.split_first().map(|(mode, rest)| (mode.as_str(), rest))
    {
        return lock_merge::merge(rest);
    }
    let mut found = BTreeMap::new();
    for path in args {
        let producers = std::fs::read(&path)
            .map(|bytes| producers(&bytes))
            .unwrap_or_default();
        found.insert(path, producers);
    }
    println!(
        "{}",
        serde_json::to_string(&found).map_err(|error| error.to_string())?
    );
    Ok(())
}
