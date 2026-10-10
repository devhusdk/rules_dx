#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

// LCOV_EXCL_START - reason: thin shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
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

fn run(argv: &[String]) -> i32 {
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
    let binary = get("--binary");
    let archive = get("--archive");
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
    let binary_sha_path = get("--binary-sha256-out");
    let archive_sha_path = get("--archive-sha256-out");
    if let Some(diagnostic) = missing {
        return dx_release_tools::bin_error(diagnostic);
    }
    let approve = match approve_raw.as_str() {
        "true" | "1" => true,
        "false" | "0" => false,
        _ => {
            return dx_release_tools::bin_error(
                "publish_dry_run_check: --approve wants true/false (dry run records approval, never publishes)",
            );
        }
    };
    let inputs = PublishDryRunInputs {
        binary: PathBuf::from(binary),
        archive: PathBuf::from(archive),
        sbom_artifact: PathBuf::from(sbom_artifact),
        spdx: PathBuf::from(spdx),
        provenance: PathBuf::from(provenance),
        notice: PathBuf::from(notice),
        notice_manifest: PathBuf::from(notice_manifest),
        notice_texts: if notice_texts_raw.is_empty() {
            Vec::new()
        } else {
            notice_texts_raw.split(':').map(PathBuf::from).collect()
        },
        release_artifacts_list: PathBuf::from(release_artifacts),
        draft_log: PathBuf::from(draft_log),
        signing_log: PathBuf::from(signing_log),
        bcr_log: PathBuf::from(bcr_log),
        human_run_log: PathBuf::from(human_run_log),
        release_tests_log: PathBuf::from(release_tests_log),
        module_bazel: PathBuf::from(module_bazel),
        verify_refusal_log: PathBuf::from(verify_refusal_log),
        approve,
    };
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
// LCOV_EXCL_STOP - reason: end thin shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
