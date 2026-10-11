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
struct ProvenanceJob {
    src: PathBuf,
    dst: PathBuf,
    builder: String,
}

fn provenance_job(argv: &[String]) -> Option<ProvenanceJob> {
    if argv.len() != 4 {
        return None;
    }
    Some(ProvenanceJob {
        src: PathBuf::from(&argv[1]),
        dst: PathBuf::from(&argv[2]),
        builder: argv[3].clone(),
    })
}

// LCOV_EXCL_START - reason: process shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
fn run(argv: &[String]) -> i32 {
    let job = match provenance_job(argv) {
        Some(job) => job,
        None => {
            return dx_release_tools::bin_usage("sbom_prov_gen", "<artifact> <out> <builder_id>");
        }
    };
    match dx_release_tools::write_provenance(&job.src, &job.dst, &job.builder) {
        Ok(()) => 0,
        Err(error) => dx_release_tools::bin_cannot_write("sbom_prov_gen", &job.dst, error),
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
            provenance_job(&argv_of(&["prog", "a.bin", "out.json", "builder"])),
            Some(ProvenanceJob {
                src: PathBuf::from("a.bin"),
                dst: PathBuf::from("out.json"),
                builder: "builder".to_owned(),
            })
        );
    }

    #[test]
    fn job_rejects_any_other_arity() {
        assert_eq!(provenance_job(&argv_of(&["prog", "a"])), None);
        assert_eq!(
            provenance_job(&argv_of(&["prog", "a", "b", "c", "d"])),
            None
        );
    }

    #[test]
    fn run_refuses_bad_arity_before_touching_the_filesystem() {
        assert_eq!(run(&argv_of(&["sbom_prov_gen"])), 1);
        assert_eq!(run(&argv_of(&["prog", "a"])), 1);
    }
}
