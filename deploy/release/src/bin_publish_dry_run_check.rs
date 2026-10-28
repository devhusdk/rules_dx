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

use dx_release_tools::cell::Expected;
use dx_release_tools::{publish_dry_run_check, PublishDryRunInputs};

fn flag_value(argv: &[String], name: &str) -> Option<String> {
    argv.windows(2).find_map(|pair| {
        if pair[0] == name {
            Some(pair[1].clone())
        } else {
            None
        }
    })
}

fn approve_of(raw: &str) -> Option<bool> {
    match raw {
        "true" | "1" => Some(true),
        "false" | "0" => Some(false),
        _ => None,
    }
}

fn notice_texts_of(raw: &str) -> Vec<PathBuf> {
    if raw.is_empty() {
        Vec::new()
    } else {
        raw.split(':').map(PathBuf::from).collect()
    }
}

struct ParsedInputs {
    inputs: PublishDryRunInputs,
    report_path: String,
}

fn parse_inputs(argv: &[String]) -> Result<ParsedInputs, String> {
    let mut missing: Option<String> = None;
    let mut get = |name: &str| -> String {
        match flag_value(argv, name) {
            Some(value) => value,
            None => {
                missing = Some(format!(
                    "publish_dry_run_check: missing required {name} <path>"
                ));
                String::new()
            }
        }
    };
    let matrix = get("--matrix");
    let records = get("--records");
    let archives = get("--archives");
    let sha = get("--sha");
    let version = get("--version");
    let workflow_ref = get("--workflow-ref");
    let sbom_artifact = get("--sbom-artifact");
    let spdx = get("--spdx");
    let provenance = get("--provenance");
    let notice = get("--notice");
    let notice_manifest = get("--notice-manifest");
    let notice_texts_raw = get("--notice-texts");
    let release_artifacts = get("--release-artifacts");
    let draft_log = get("--draft-log");
    let signing_log = get("--signing-log");
    let bcr_log = get("--bcr-log");
    let human_run_log = get("--human-run-log");
    let release_tests_log = get("--release-tests-log");
    let module_bazel = get("--module-bazel");
    let verify_refusal_log = get("--verify-refusal-log");
    let approve_raw = get("--approve");
    let report_path = get("--report");
    if let Some(diagnostic) = missing {
        return Err(diagnostic);
    }
    let approve = approve_of(&approve_raw).ok_or_else(|| {
        "publish_dry_run_check: --approve wants true/false (dry run records approval, never publishes)".to_owned()
    })?;
    Ok(ParsedInputs {
        inputs: PublishDryRunInputs {
            matrix: PathBuf::from(matrix),
            records: PathBuf::from(records),
            archives: PathBuf::from(archives),
            expected: Expected {
                sha,
                version,
                workflow_ref,
            },
            sbom_artifact: PathBuf::from(sbom_artifact),
            spdx: PathBuf::from(spdx),
            provenance: PathBuf::from(provenance),
            notice: PathBuf::from(notice),
            notice_manifest: PathBuf::from(notice_manifest),
            notice_texts: notice_texts_of(&notice_texts_raw),
            release_artifacts_list: PathBuf::from(release_artifacts),
            draft_log: PathBuf::from(draft_log),
            signing_log: PathBuf::from(signing_log),
            bcr_log: PathBuf::from(bcr_log),
            human_run_log: PathBuf::from(human_run_log),
            release_tests_log: PathBuf::from(release_tests_log),
            module_bazel: PathBuf::from(module_bazel),
            verify_refusal_log: PathBuf::from(verify_refusal_log),
            approve,
        },
        report_path,
    })
}

// LCOV_EXCL_START - reason: process shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
fn run(argv: &[String]) -> i32 {
    let parsed = match parse_inputs(argv) {
        Ok(parsed) => parsed,
        Err(diagnostic) => return dx_release_tools::bin_error(diagnostic),
    };
    match publish_dry_run_check(&parsed.inputs) {
        Ok(outcome) => {
            if let Err(error) = std::fs::write(&parsed.report_path, outcome.report.as_bytes()) {
                return dx_release_tools::bin_cannot_write(
                    "publish_dry_run_check",
                    Path::new(&parsed.report_path),
                    error,
                );
            }
            print!(
                "{}sbom subject digest: {} (linkage ok: true)\npackaging ok: true (SBOM pair plus NOTICE signed together)\ndraft dry-run ok: true\nsigning dry-run ok: true\nbcr dry-run ok: true\napproved: {}; published: False (dry run never publishes)\n",
                outcome.verification, outcome.sbom_digest, parsed.inputs.approve,
            );
            if outcome.qualified {
                0
            } else {
                eprintln!(
                    "publish_dry_run_check: not every release cell qualified (see the report)"
                );
                1
            }
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

    fn full_argv_without(name: &str) -> Vec<String> {
        let mut argv = Vec::new();
        let mut pairs = full_argv("1").into_iter();
        argv.push(pairs.next().expect("program name"));
        while let (Some(flag), Some(value)) = (pairs.next(), pairs.next()) {
            if flag != name {
                argv.push(flag);
                argv.push(value);
            }
        }
        argv
    }

    fn full_argv(approve: &str) -> Vec<String> {
        argv_of(&[
            ("--matrix", "matrix"),
            ("--records", "records"),
            ("--archives", "archives"),
            ("--sha", "sha"),
            ("--version", "version"),
            ("--workflow-ref", "ref"),
            ("--sbom-artifact", "sbom"),
            ("--spdx", "spdx"),
            ("--provenance", "provenance"),
            ("--notice", "notice"),
            ("--notice-manifest", "manifest"),
            ("--notice-texts", "a:b"),
            ("--release-artifacts", "artifacts"),
            ("--draft-log", "draft"),
            ("--signing-log", "signing"),
            ("--bcr-log", "bcr"),
            ("--human-run-log", "human"),
            ("--release-tests-log", "tests"),
            ("--module-bazel", "module"),
            ("--verify-refusal-log", "refusal"),
            ("--approve", approve),
            ("--report", "report"),
        ])
    }

    #[test]
    fn flag_value_reads_the_pair_after_a_name() {
        let argv = argv_of(&[("--matrix", "staged")]);
        assert_eq!(flag_value(&argv, "--matrix"), Some("staged".to_owned()));
        assert_eq!(flag_value(&argv, "--records"), None);
    }

    #[test]
    fn flag_value_ignores_a_trailing_name_without_value() {
        let argv = vec!["publish_dry_run_check".to_owned(), "--matrix".to_owned()];
        assert_eq!(flag_value(&argv, "--matrix"), None);
    }

    #[test]
    fn approve_accepts_spelled_and_numeric_forms() {
        assert_eq!(approve_of("true"), Some(true));
        assert_eq!(approve_of("1"), Some(true));
        assert_eq!(approve_of("false"), Some(false));
        assert_eq!(approve_of("0"), Some(false));
    }

    #[test]
    fn approve_rejects_anything_else() {
        for raw in ["", "True", "yes", "2", "true "] {
            assert_eq!(approve_of(raw), None, "{raw:?}");
        }
    }

    #[test]
    fn notice_texts_splits_colon_lists_and_keeps_empty_empty() {
        assert!(notice_texts_of("").is_empty());
        assert_eq!(notice_texts_of("a"), vec![PathBuf::from("a")]);
        assert_eq!(
            notice_texts_of("a:b"),
            vec![PathBuf::from("a"), PathBuf::from("b")]
        );
    }

    #[test]
    fn empty_argv_reports_the_last_missing_flag() {
        let error = parse_inputs(&["publish_dry_run_check".to_owned()])
            .err()
            .expect("missing flag must fail");
        assert_eq!(
            error,
            "publish_dry_run_check: missing required --report <path>"
        );
    }

    #[test]
    fn single_missing_flag_names_that_flag() {
        let error = parse_inputs(&full_argv_without("--records"))
            .err()
            .expect("missing flag must fail");
        assert_eq!(
            error,
            "publish_dry_run_check: missing required --records <path>"
        );
    }

    #[test]
    fn invalid_approve_fails_before_any_filesystem_use() {
        let error = parse_inputs(&full_argv("maybe"))
            .err()
            .expect("bad approve must fail");
        assert_eq!(
            error,
            "publish_dry_run_check: --approve wants true/false (dry run records approval, never publishes)"
        );
    }

    #[test]
    fn valid_argv_assembles_inputs_with_split_texts() {
        let parsed = parse_inputs(&full_argv("1")).unwrap();
        assert_eq!(parsed.inputs.matrix, PathBuf::from("matrix"));
        assert_eq!(parsed.inputs.expected.version, "version");
        assert_eq!(parsed.inputs.expected.workflow_ref, "ref");
        assert!(parsed.inputs.approve);
        assert_eq!(
            parsed.inputs.notice_texts,
            vec![PathBuf::from("a"), PathBuf::from("b")]
        );
        assert_eq!(parsed.report_path, "report");
    }

    #[test]
    fn run_without_flags_exits_nonzero_without_touching_files() {
        assert_eq!(run(&["publish_dry_run_check".to_owned()]), 1);
    }

    #[test]
    fn run_with_invalid_approve_exits_nonzero_before_any_io() {
        assert_eq!(run(&full_argv("maybe")), 1);
    }
}
