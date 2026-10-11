#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use std::path::{Path, PathBuf};

use dx_release_tools::{publish_dry_run_check, sha256sum_line, PublishDryRunInputs};

fn flag_value(argv: &[String], name: &str) -> Option<String> {
    argv.windows(2).find_map(|pair| {
        if pair[0] == name {
            Some(pair[1].clone())
        } else {
            None
        }
    })
}

fn flag_or_empty(argv: &[String], name: &str) -> String {
    flag_value(argv, name).unwrap_or_default()
}

fn approve_of(raw: &str) -> Result<bool, String> {
    match raw {
        "true" | "1" => Ok(true),
        "false" | "0" => Ok(false),
        _ => Err(
            "publish_dry_run_check: --approve wants true/false (dry run records approval, never publishes)"
                .to_owned(),
        ),
    }
}

fn split_texts(raw: &str) -> Vec<PathBuf> {
    if raw.is_empty() {
        Vec::new()
    } else {
        raw.split(':').map(PathBuf::from).collect()
    }
}

const REQUIRED_FLAGS: &[&str] = &[
    "--binary",
    "--archive",
    "--sbom-artifact",
    "--spdx",
    "--provenance",
    "--notice",
    "--notice-manifest",
    "--notice-texts",
    "--release-artifacts",
    "--draft-log",
    "--signing-log",
    "--bcr-log",
    "--human-run-log",
    "--release-tests-log",
    "--module-bazel",
    "--verify-refusal-log",
    "--approve",
    "--report",
    "--binary-sha256-out",
    "--archive-sha256-out",
];

fn missing_required(argv: &[String]) -> Option<String> {
    REQUIRED_FLAGS.iter().find_map(|name| {
        if flag_value(argv, name).is_none() {
            Some(format!(
                "publish_dry_run_check: missing required {name} <path>"
            ))
        } else {
            None
        }
    })
}

// LCOV_EXCL_START - reason: process shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
fn run(argv: &[String]) -> i32 {
    if let Some(diagnostic) = missing_required(argv) {
        return dx_release_tools::bin_error(diagnostic);
    }
    let approve = match approve_of(&flag_or_empty(argv, "--approve")) {
        Ok(approve) => approve,
        Err(diagnostic) => return dx_release_tools::bin_error(diagnostic),
    };
    let inputs = PublishDryRunInputs {
        binary: PathBuf::from(flag_or_empty(argv, "--binary")),
        archive: PathBuf::from(flag_or_empty(argv, "--archive")),
        sbom_artifact: PathBuf::from(flag_or_empty(argv, "--sbom-artifact")),
        spdx: PathBuf::from(flag_or_empty(argv, "--spdx")),
        provenance: PathBuf::from(flag_or_empty(argv, "--provenance")),
        notice: PathBuf::from(flag_or_empty(argv, "--notice")),
        notice_manifest: PathBuf::from(flag_or_empty(argv, "--notice-manifest")),
        notice_texts: split_texts(&flag_or_empty(argv, "--notice-texts")),
        release_artifacts_list: PathBuf::from(flag_or_empty(argv, "--release-artifacts")),
        draft_log: PathBuf::from(flag_or_empty(argv, "--draft-log")),
        signing_log: PathBuf::from(flag_or_empty(argv, "--signing-log")),
        bcr_log: PathBuf::from(flag_or_empty(argv, "--bcr-log")),
        human_run_log: PathBuf::from(flag_or_empty(argv, "--human-run-log")),
        release_tests_log: PathBuf::from(flag_or_empty(argv, "--release-tests-log")),
        module_bazel: PathBuf::from(flag_or_empty(argv, "--module-bazel")),
        verify_refusal_log: PathBuf::from(flag_or_empty(argv, "--verify-refusal-log")),
        approve,
    };
    let report_path = flag_or_empty(argv, "--report");
    let binary_sha_path = flag_or_empty(argv, "--binary-sha256-out");
    let archive_sha_path = flag_or_empty(argv, "--archive-sha256-out");
    match publish_dry_run_check(&inputs) {
        Ok(outcome) => {
            if let Err(error) = std::fs::write(&report_path, outcome.report.as_bytes()) {
                return dx_release_tools::bin_cannot_write(
                    "publish_dry_run_check",
                    Path::new(&report_path),
                    error,
                );
            }
            let binary_line = sha256sum_line(&outcome.binary_digest, "dx-linux-x86_64");
            if let Err(error) = std::fs::write(&binary_sha_path, binary_line.as_bytes()) {
                return dx_release_tools::bin_cannot_write(
                    "publish_dry_run_check",
                    Path::new(&binary_sha_path),
                    error,
                );
            }
            let archive_line = sha256sum_line(&outcome.archive_digest, "dx-standalone.tar.gz");
            if let Err(error) = std::fs::write(&archive_sha_path, archive_line.as_bytes()) {
                return dx_release_tools::bin_cannot_write(
                    "publish_dry_run_check",
                    Path::new(&archive_sha_path),
                    error,
                );
            }
            print!(
                "seed binary sha256: {}\nstandalone archive sha256: {}\nsbom subject digest: {} (linkage ok: true)\npackaging ok: true (SBOM pair plus NOTICE signed together)\ndraft dry-run ok: true\nsigning dry-run ok: true\nbcr dry-run ok: true\napproved: {}; published: False (dry run never publishes)\n",
                outcome.binary_digest, outcome.archive_digest, outcome.sbom_digest, approve,
            );
            0
        }
        Err(diagnostic) => dx_release_tools::bin_error(diagnostic),
    }
}

fn main() {
    std::process::exit(run(&std::env::args().collect::<Vec<_>>()));
}
// LCOV_EXCL_STOP - reason: end process shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md

#[cfg(test)]
mod tests {
    use super::*;

    fn argv_of(pairs: &[(&str, &str)]) -> Vec<String> {
        let mut argv = vec!["publish_dry_run_check".to_owned()];
        for (name, value) in pairs {
            argv.push((*name).to_owned());
            argv.push((*value).to_owned());
        }
        argv
    }

    fn full_argv() -> Vec<String> {
        argv_of(&[
            ("--binary", "dx"),
            ("--archive", "dx.tar.gz"),
            ("--sbom-artifact", "dx.tar.gz"),
            ("--spdx", "sbom.spdx.json"),
            ("--provenance", "sbom.provenance.json"),
            ("--notice", "NOTICE"),
            ("--notice-manifest", "inventory.txt"),
            ("--notice-texts", "a.txt:b.txt"),
            ("--release-artifacts", "artifacts.txt"),
            ("--draft-log", "draft.log"),
            ("--signing-log", "signing.log"),
            ("--bcr-log", "bcr.log"),
            ("--human-run-log", "human.log"),
            ("--release-tests-log", "tests.log"),
            ("--module-bazel", "MODULE.bazel"),
            ("--verify-refusal-log", "refusal.log"),
            ("--approve", "true"),
            ("--report", "report.json"),
            ("--binary-sha256-out", "dx.sha256"),
            ("--archive-sha256-out", "dx.tar.gz.sha256"),
        ])
    }

    #[test]
    fn flag_value_reads_the_first_matching_pair() {
        let argv = argv_of(&[("--binary", "a"), ("--binary", "b")]);
        assert_eq!(flag_value(&argv, "--binary"), Some("a".to_owned()));
        assert_eq!(flag_value(&argv, "--missing"), None);
    }

    #[test]
    fn flag_value_ignores_a_dangling_trailing_name() {
        let argv = vec!["publish_dry_run_check".to_owned(), "--binary".to_owned()];
        assert_eq!(flag_value(&argv, "--binary"), None);
    }

    #[test]
    fn approval_accepts_true_and_false_spellings() {
        assert_eq!(approve_of("true"), Ok(true));
        assert_eq!(approve_of("1"), Ok(true));
        assert_eq!(approve_of("false"), Ok(false));
        assert_eq!(approve_of("0"), Ok(false));
    }

    #[test]
    fn approval_rejects_anything_else() {
        assert_eq!(
            approve_of("maybe"),
            Err(
                "publish_dry_run_check: --approve wants true/false (dry run records approval, never publishes)"
                    .to_owned()
            )
        );
    }

    #[test]
    fn notice_texts_split_on_colons() {
        assert!(split_texts("").is_empty());
        assert_eq!(split_texts("a.txt"), vec![PathBuf::from("a.txt")]);
        assert_eq!(
            split_texts("a.txt:b.txt"),
            vec![PathBuf::from("a.txt"), PathBuf::from("b.txt")]
        );
    }

    #[test]
    fn missing_required_names_the_first_gap_in_flag_order() {
        assert_eq!(
            missing_required(&["publish_dry_run_check".to_owned()]),
            Some("publish_dry_run_check: missing required --binary <path>".to_owned())
        );
        let mut argv = full_argv();
        argv.pop();
        argv.pop();
        assert_eq!(
            missing_required(&argv),
            Some("publish_dry_run_check: missing required --archive-sha256-out <path>".to_owned())
        );
        assert_eq!(missing_required(&full_argv()), None);
    }

    #[test]
    fn run_refuses_before_touching_the_filesystem() {
        assert_eq!(run(&["publish_dry_run_check".to_owned()]), 1);
        let mut argv = full_argv();
        let approve = argv
            .iter()
            .position(|item| item == "--approve")
            .unwrap_or(0)
            + 1;
        argv[approve] = "maybe".to_owned();
        assert_eq!(run(&argv), 1);
    }

    #[test]
    fn run_with_missing_staged_files_fails_without_writing() {
        assert_eq!(run(&full_argv()), 1);
        assert!(!std::path::Path::new("report.json").exists());
    }
}
