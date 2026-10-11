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
struct SpdxJob {
    src: PathBuf,
    dst: PathBuf,
    package: String,
    supplier: String,
}

fn spdx_job(argv: &[String]) -> Option<SpdxJob> {
    if argv.len() != 5 {
        return None;
    }
    Some(SpdxJob {
        src: PathBuf::from(&argv[1]),
        dst: PathBuf::from(&argv[2]),
        package: argv[3].clone(),
        supplier: argv[4].clone(),
    })
}

// LCOV_EXCL_START - reason: process shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
fn run(argv: &[String]) -> i32 {
    let job = match spdx_job(argv) {
        Some(job) => job,
        None => {
            return dx_release_tools::bin_usage(
                "sbom_spdx_gen",
                "<artifact> <out> <package> <supplier>",
            );
        }
    };
    match dx_release_tools::write_spdx(&job.src, &job.dst, &job.package, &job.supplier) {
        Ok(()) => 0,
        Err(error) => dx_release_tools::bin_cannot_write("sbom_spdx_gen", &job.dst, error),
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
            spdx_job(&argv_of(&["prog", "a.bin", "out.json", "pkg", "sup"])),
            Some(SpdxJob {
                src: PathBuf::from("a.bin"),
                dst: PathBuf::from("out.json"),
                package: "pkg".to_owned(),
                supplier: "sup".to_owned(),
            })
        );
    }

    #[test]
    fn job_rejects_any_other_arity() {
        assert_eq!(spdx_job(&argv_of(&["prog", "a", "b", "c"])), None);
        assert_eq!(spdx_job(&argv_of(&["prog", "a", "b", "c", "d", "e"])), None);
    }

    #[test]
    fn run_refuses_bad_arity_before_touching_the_filesystem() {
        assert_eq!(run(&argv_of(&["sbom_spdx_gen"])), 1);
        assert_eq!(run(&argv_of(&["prog", "a", "b"])), 1);
    }
}
