#![deny(clippy::indexing_slicing, clippy::string_slice)]
#![expect(
    clippy::disallowed_methods,
    reason = "runs local tools that exit on their own"
)]

mod blocker_refs;
mod evidence;
mod evidence_inputs;
mod lock_merge;
mod pr_check;
mod pr_links;
mod release_cell;
mod review_paths;
mod table;
mod test_log_tails;
mod upgrade_e2e;

use dx_binfmt::producers;
use std::collections::BTreeMap;

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.split_first().map(|(mode, rest)| (mode.as_str(), rest)) {
        Some(("--evidence", rest)) => return evidence::append(rest),
        Some(("--evidence-inputs", rest)) => return evidence_inputs::check(rest),
        Some(("--table", rest)) => return table::print(rest),
        Some(("--lock-merge", rest)) => return lock_merge::merge(rest),
        Some(("--pr-links", rest)) => return pr_links::check(rest),
        Some(("--test-log-tails", rest)) => return test_log_tails::print(rest),
        _ => {}
    }
    if let Some(("--blocker-refs", rest)) =
        args.split_first().map(|(mode, rest)| (mode.as_str(), rest))
    {
        return blocker_refs::print(rest);
    }
    if let Some(("--upgrade-e2e", rest)) =
        args.split_first().map(|(mode, rest)| (mode.as_str(), rest))
    {
        return upgrade_e2e::e2e(rest);
    }
    if let Some(("--pr-check", rest)) = args.split_first().map(|(mode, rest)| (mode.as_str(), rest))
    {
        return pr_check::check(rest);
    }
    if let Some(("--release-cell", rest)) =
        args.split_first().map(|(mode, rest)| (mode.as_str(), rest))
    {
        return release_cell::cell(rest);
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
