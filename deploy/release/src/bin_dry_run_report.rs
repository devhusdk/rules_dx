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
use std::path::PathBuf;

use dx_release_tools::DryRunInputs;

const USAGE: &str = "--stage <dir> --approve <true|false> --binary <path> --standalone-archive <path> --release-artifacts <path> --spdx <path> --provenance <path> --notice <path> --sbom-artifact <path> --notice-manifest <path> [--notice-text <path>]... --draft-log <path> --tests-log <path> --signing-log <path> --bcr-log <path> --driver-log <path> --verify-refusal-log <path> --module-file <path>";

fn usage() -> i32 {
    dx_release_tools::bin_usage("dry_run_report", USAGE)
}

fn take(argv: &[String], index: &mut usize) -> Option<String> {
    *index += 1;
    argv.get(*index).cloned()
}

fn run(argv: &[String]) -> i32 {
    let mut stage: Option<PathBuf> = None;
    let mut approve: Option<bool> = None;
    let mut binary: Option<PathBuf> = None;
    let mut standalone_archive: Option<PathBuf> = None;
    let mut release_artifacts: Option<PathBuf> = None;
    let mut spdx: Option<PathBuf> = None;
    let mut provenance: Option<PathBuf> = None;
    let mut notice: Option<PathBuf> = None;
    let mut sbom_artifact: Option<PathBuf> = None;
    let mut notice_manifest: Option<PathBuf> = None;
    let mut notice_texts: Vec<PathBuf> = Vec::new();
    let mut draft_log: Option<PathBuf> = None;
    let mut tests_log: Option<PathBuf> = None;
    let mut signing_log: Option<PathBuf> = None;
    let mut bcr_log: Option<PathBuf> = None;
    let mut driver_log: Option<PathBuf> = None;
    let mut verify_refusal_log: Option<PathBuf> = None;
    let mut module_file: Option<PathBuf> = None;
    let mut index = 1;
    while index < argv.len() {
        let flag = argv[index].as_str();
        match flag {
            "--stage" => stage = take(argv, &mut index).map(PathBuf::from),
            "--binary" => binary = take(argv, &mut index).map(PathBuf::from),
            "--standalone-archive" => {
                standalone_archive = take(argv, &mut index).map(PathBuf::from);
            }
            "--release-artifacts" => {
                release_artifacts = take(argv, &mut index).map(PathBuf::from);
            }
            "--spdx" => spdx = take(argv, &mut index).map(PathBuf::from),
            "--provenance" => provenance = take(argv, &mut index).map(PathBuf::from),
            "--notice" => notice = take(argv, &mut index).map(PathBuf::from),
            "--sbom-artifact" => sbom_artifact = take(argv, &mut index).map(PathBuf::from),
            "--notice-manifest" => {
                notice_manifest = take(argv, &mut index).map(PathBuf::from);
            }
            "--notice-text" => {
                if let Some(path) = take(argv, &mut index) {
                    notice_texts.push(PathBuf::from(path));
                }
            }
            "--draft-log" => draft_log = take(argv, &mut index).map(PathBuf::from),
            "--tests-log" => tests_log = take(argv, &mut index).map(PathBuf::from),
            "--signing-log" => signing_log = take(argv, &mut index).map(PathBuf::from),
            "--bcr-log" => bcr_log = take(argv, &mut index).map(PathBuf::from),
            "--driver-log" => driver_log = take(argv, &mut index).map(PathBuf::from),
            "--verify-refusal-log" => {
                verify_refusal_log = take(argv, &mut index).map(PathBuf::from);
            }
            "--module-file" => module_file = take(argv, &mut index).map(PathBuf::from),
            "--approve" => {
                approve = match take(argv, &mut index).as_deref() {
                    Some("true") => Some(true),
                    Some("false") => Some(false),
                    _ => return usage(),
                };
            }
            "--help" => return usage(),
            _ => return usage(),
        }
        index += 1;
    }
    let inputs = DryRunInputs {
        approve: match approve {
            Some(value) => value,
            None => return usage(),
        },
        binary: match binary {
            Some(path) => path,
            None => return usage(),
        },
        standalone_archive: match standalone_archive {
            Some(path) => path,
            None => return usage(),
        },
        release_artifacts: match release_artifacts {
            Some(path) => path,
            None => return usage(),
        },
        spdx: match spdx {
            Some(path) => path,
            None => return usage(),
        },
        provenance: match provenance {
            Some(path) => path,
            None => return usage(),
        },
        notice: match notice {
            Some(path) => path,
            None => return usage(),
        },
        sbom_artifact: match sbom_artifact {
            Some(path) => path,
            None => return usage(),
        },
        notice_manifest: match notice_manifest {
            Some(path) => path,
            None => return usage(),
        },
        notice_texts,
        draft_log: match draft_log {
            Some(path) => path,
            None => return usage(),
        },
        tests_log: match tests_log {
            Some(path) => path,
            None => return usage(),
        },
        signing_log: match signing_log {
            Some(path) => path,
            None => return usage(),
        },
        bcr_log: match bcr_log {
            Some(path) => path,
            None => return usage(),
        },
        driver_log: match driver_log {
            Some(path) => path,
            None => return usage(),
        },
        verify_refusal_log: match verify_refusal_log {
            Some(path) => path,
            None => return usage(),
        },
        module_file: match module_file {
            Some(path) => path,
            None => return usage(),
        },
    };
    let out_dir = match stage {
        Some(path) => path,
        None => return usage(),
    };
    let report = match dx_release_tools::dry_run_check(&inputs) {
        Ok(report) => report,
        Err(diagnostic) => return dx_release_tools::bin_error(diagnostic),
    };
    match dx_release_tools::write_dry_run_outputs(&out_dir, &report) {
        Ok(()) => {
            print!("{}", dx_release_tools::render_dry_run_console(&report));
            0
        }
        Err(error) => dx_release_tools::bin_cannot_write("dry_run_report", &out_dir, error),
    }
}

fn main() {
    std::process::exit(run(&std::env::args().collect::<Vec<_>>()));
}
// LCOV_EXCL_STOP - reason: end thin shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
