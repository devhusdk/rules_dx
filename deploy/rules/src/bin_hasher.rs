#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use std::path::PathBuf;

#[derive(Debug, PartialEq, Eq)]
struct HashJob {
    src: PathBuf,
    dst: PathBuf,
}

fn hash_job(argv: &[String]) -> Option<HashJob> {
    if argv.len() != 3 {
        return None;
    }
    Some(HashJob {
        src: PathBuf::from(&argv[1]),
        dst: PathBuf::from(&argv[2]),
    })
}

// LCOV_EXCL_START - reason: process shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
fn run(argv: &[String]) -> i32 {
    let job = match hash_job(argv) {
        Some(job) => job,
        None => return dx_deploy_tools::bin_usage("hasher", "<src> <out>"),
    };
    match dx_deploy_tools::hash_file(&job.src, &job.dst) {
        Ok(()) => 0,
        Err(error) => dx_deploy_tools::bin_cannot("hasher", "hash", &job.src, error),
    }
}

fn main() {
    std::process::exit(run(&std::env::args().collect::<Vec<_>>()));
}
// LCOV_EXCL_STOP - reason: end process shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md

#[cfg(test)]
mod tests {
    use super::*;

    fn argv_of(items: &[&str]) -> Vec<String> {
        items.iter().map(|item| (*item).to_owned()).collect()
    }

    #[test]
    fn job_binds_each_positional() {
        assert_eq!(
            hash_job(&argv_of(&["prog", "a.bin", "a.sha256"])),
            Some(HashJob {
                src: PathBuf::from("a.bin"),
                dst: PathBuf::from("a.sha256"),
            })
        );
    }

    #[test]
    fn job_rejects_any_other_arity() {
        assert_eq!(hash_job(&argv_of(&["prog", "a"])), None);
        assert_eq!(hash_job(&argv_of(&["prog", "a", "b", "c"])), None);
    }

    #[test]
    fn run_refuses_bad_arity_before_touching_the_filesystem() {
        assert_eq!(run(&argv_of(&["hasher"])), 1);
        assert_eq!(run(&argv_of(&["prog", "a"])), 1);
    }
}
