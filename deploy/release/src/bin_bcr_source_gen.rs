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
struct BcrJob {
    dst: PathBuf,
    module: String,
    version: String,
}

fn bcr_job(argv: &[String]) -> Option<BcrJob> {
    if argv.len() != 4 {
        return None;
    }
    Some(BcrJob {
        dst: PathBuf::from(&argv[1]),
        module: argv[2].clone(),
        version: argv[3].clone(),
    })
}

// LCOV_EXCL_START - reason: process shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
fn run(argv: &[String]) -> i32 {
    let job = match bcr_job(argv) {
        Some(job) => job,
        None => {
            return dx_release_tools::bin_usage("bcr_source_gen", "<out> <module> <version>");
        }
    };
    match dx_release_tools::write_bcr_source(&job.dst, &job.module, &job.version) {
        Ok(()) => 0,
        Err(error) => dx_release_tools::bin_cannot_write("bcr_source_gen", &job.dst, error),
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
            bcr_job(&argv_of(&["prog", "out.bzl", "rules_dx", "0.0.0"])),
            Some(BcrJob {
                dst: PathBuf::from("out.bzl"),
                module: "rules_dx".to_owned(),
                version: "0.0.0".to_owned(),
            })
        );
    }

    #[test]
    fn job_rejects_any_other_arity() {
        assert_eq!(bcr_job(&argv_of(&["prog", "a"])), None);
        assert_eq!(bcr_job(&argv_of(&["prog", "a", "b", "c", "d"])), None);
    }

    #[test]
    fn run_refuses_bad_arity_before_touching_the_filesystem() {
        assert_eq!(run(&argv_of(&["bcr_source_gen"])), 1);
        assert_eq!(run(&argv_of(&["prog", "a"])), 1);
    }
}
