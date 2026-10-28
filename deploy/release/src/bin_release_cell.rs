#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use dx_release_tools::cell::{self, CellRecord, Verdict};

/// Returns the exit code and message for one finished `qualify`.
fn qualify_outcome(record: &CellRecord) -> (i32, String) {
    let failed: Vec<String> = record
        .checks
        .iter()
        .filter(|check| check.verdict != Verdict::Ok)
        .map(|check| format!("{} {:?}: {}", check.name, check.verdict, check.detail))
        .collect();
    if record.result == Verdict::Ok {
        return (0, format!("release_cell: {} qualified\n", record.cell));
    }
    (
        1,
        format!(
            "release_cell: {} not qualified\n{}\n",
            record.cell,
            failed.join("\n")
        ),
    )
}

// LCOV_EXCL_START - reason: process shim with the real command runner, issue: 1386, policy: docs/cli/commands/build-test-coverage.md
struct CommandRunner;

impl cell::Runner for CommandRunner {
    fn run(
        &self,
        program: &std::path::Path,
        args: &[&str],
        cwd: &std::path::Path,
    ) -> cell::RunOutput {
        match std::process::Command::new(program)
            .args(args)
            .current_dir(cwd)
            .output()
        {
            Ok(out) => cell::RunOutput {
                success: out.status.success(),
                stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
                stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
            },
            Err(error) => cell::RunOutput {
                success: false,
                stdout: String::new(),
                stderr: error.to_string(),
            },
        }
    }
}

fn usage() -> i32 {
    eprintln!("usage: release_cell {}", cell::QUALIFY_USAGE);
    eprintln!("       release_cell {}", cell::VERIFY_USAGE);
    eprintln!("       release_cell {}", cell::VERSION_USAGE);
    1
}

fn run(argv: &[String]) -> i32 {
    match argv.get(1).map(String::as_str) {
        Some("qualify") => match cell::qualify_main(&argv[2..], &CommandRunner) {
            Ok(record) => {
                let (code, text) = qualify_outcome(&record);
                eprint!("{text}");
                code
            }
            Err(error) => dx_release_tools::bin_error(format!("release_cell: {error}")),
        },
        Some("verify") => match cell::verify_main(&argv[2..]) {
            Ok((ok, text)) => {
                print!("{text}");
                i32::from(!ok)
            }
            Err(error) => dx_release_tools::bin_error(format!("release_cell: {error}")),
        },
        Some("version") => match cell::version_main(&argv[2..], &CommandRunner) {
            Ok(version) => {
                println!("{version}");
                0
            }
            Err(error) => dx_release_tools::bin_error(format!("release_cell: {error}")),
        },
        _ => usage(),
    }
}

fn main() {
    std::process::exit(run(&std::env::args().collect::<Vec<_>>()));
}
// LCOV_EXCL_STOP - reason: end process shim, issue: 1386, policy: docs/cli/commands/build-test-coverage.md

#[cfg(test)]
mod tests {
    use super::*;

    fn record(result: Verdict) -> CellRecord {
        serde_json::from_value(serde_json::json!({
            "schema": cell::RECORD_SCHEMA,
            "cell": "dx-linux-x86_64",
            "archive": {"name": "dx-linux-x86_64.tar.gz", "sha256": "", "bytes": 0},
            "binary": {"sha256": "", "format": "elf", "machine": "x86_64", "abi": "gnu", "needed": []},
            "members": [],
            "sha": "", "version": "", "workflow_ref": "", "workflow_sha": "",
            "run_id": "", "job": "",
            "runner": {"os": "linux", "arch": "x86_64"},
            "checks": [
                {"name": "run", "verdict": "ok", "detail": ""},
                {"name": "consumer", "verdict": "skipped", "detail": "no --consumer"},
            ],
            "result": result,
        }))
        .expect("record")
    }

    #[test]
    fn qualify_outcome_exits_zero_only_when_qualified() {
        assert_eq!(
            qualify_outcome(&record(Verdict::Ok)),
            (0, "release_cell: dx-linux-x86_64 qualified\n".to_owned())
        );
        assert_eq!(
            qualify_outcome(&record(Verdict::Failed)),
            (
                1,
                "release_cell: dx-linux-x86_64 not qualified\nconsumer Skipped: no --consumer\n"
                    .to_owned()
            )
        );
    }
}
