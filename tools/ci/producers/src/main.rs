//! `producers_bin` prints the compiler signatures of each file argument as a JSON object.
//! `producers_bin --evidence OUT PLATFORM FACTS ROW_LABELS CONSUMER_LABELS` appends the compiler evidence lines to OUT.
//! `producers_bin --evidence-inputs` validates the evidence workflow dispatch inputs and appends them to `GITHUB_OUTPUT`.
//! `producers_bin --lock-merge LOCKS_DIR OUT` merges the per-platform module locks into OUT.
//! `producers_bin --blocker-refs OWNER/REPO` prints the references of a "Blocked on:" line on stdin, marking those that wait on a release.
//! `producers_bin --review-paths [PREFIX_FILE]` prints the stdin paths that start with a listed prefix.
//! `producers_bin --upgrade-e2e [--from VERSION] [--to VERSION] [--resolve-only]` upgrades a throwaway consumer between `rules_dx` versions and back.
//! `producers_bin --pr-links [COMMIT_MESSAGES_FILE]` fails when the pull request body in `GITHUB_EVENT_PATH` misses an issue link.
//! `producers_bin --pr-check SET [BAZEL_FLAGS...]` runs the analysis-only pull request checks of SET, or of every set for `all`.
//! `producers_bin --release-cell OUT` builds and qualifies the release cell named by CELL and the workflow environment.
//! `producers_bin --table EVIDENCE_DIR OUT PLATFORM...` merges the evidence under a directory into OUT and prints the table.
//! `producers_bin --test-log-tails (--bep FILE | --dx FILE)...` prints the test.log tail of each failing test as a job log group.

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
