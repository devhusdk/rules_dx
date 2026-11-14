//! `producers_bin` prints the compiler signatures of each file argument as a JSON object.
//! `producers_bin --evidence OUT PLATFORM FACTS ROW_LABELS CONSUMER_LABELS` appends the compiler evidence lines to OUT.
//! `producers_bin --lock-merge LOCKS_DIR OUT` merges the per-platform module locks into OUT.
//! `producers_bin --blocker-refs OWNER/REPO` prints the references of a "Blocked on:" line on stdin.
//! `producers_bin --review-paths [PREFIX_FILE]` prints the stdin paths that start with a listed prefix.

#![deny(clippy::indexing_slicing, clippy::string_slice)]

mod blocker_refs;
mod evidence;
mod lock_merge;
mod review_paths;

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
    if let Some(("--blocker-refs", rest)) =
        args.split_first().map(|(mode, rest)| (mode.as_str(), rest))
    {
        return blocker_refs::print(rest);
    }
    if let Some(("--review-paths", rest)) =
        args.split_first().map(|(mode, rest)| (mode.as_str(), rest))
    {
        return review_paths::filter(rest);
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
