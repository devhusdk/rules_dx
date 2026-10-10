mod capabilities;
mod completion;
mod hooks;
mod init;
mod inspect;
mod new;
mod status;
#[cfg(test)]
mod test_support;
mod upgrade;
mod version;
pub(crate) mod watch;

use std::io::Write;

use crate::args::{Command, Invocation};
use crate::resolve::QueryRunner;
use dx_output::OutputMode;

pub(crate) use crate::exec::common::{operational, pre_exec};

pub struct AdoptEnv<'a> {
    pub workspace: &'a std::path::Path,
    pub query_runner: &'a dyn QueryRunner,
    pub runner: &'a dyn dx_process::Runner,
    pub out: &'a mut dyn Write,
    pub err: &'a mut dyn Write,
}

fn summaries_suppressed(invocation: &Invocation) -> bool {
    invocation.quiet || matches!(invocation.output, OutputMode::Text { quiet: true })
}

pub fn execute_adoption(invocation: &Invocation, env: AdoptEnv<'_>) -> i32 {
    let AdoptEnv {
        workspace,
        query_runner,
        runner,
        out,
        err,
    } = env;
    match invocation.command {
        Command::Init => init::execute_init(invocation, workspace, out, err),
        Command::New => new::execute_new(invocation, workspace, out, err),
        Command::Upgrade => upgrade::execute_upgrade(invocation, workspace, out, err),
        Command::Hooks => {
            hooks::execute_hooks(invocation, workspace, query_runner, runner, None, out, err)
        }
        Command::Status => status::execute_status(invocation, workspace, out, err),
        Command::Capabilities => {
            capabilities::execute_capabilities(invocation, workspace, out, err)
        }
        Command::Version => version::execute_version(invocation, workspace, out, err),
        Command::Owners | Command::Deps | Command::Why => {
            inspect::execute_inspect(invocation, workspace, query_runner, out, err)
        }
        Command::Completion => completion::execute_completion(invocation, out, err),
        _ => pre_exec(err, "not an adoption command"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adopt::test_support::{env, invocation, run, Truncated};

    #[test]
    fn status_missing_pin_json_truncation_never_reports_success() {
        let scratch = dx_test_scratch::scratch("status-missing-pipe-");
        let inv = invocation(&["status", "--output=json"]);
        let run =
            |out: &mut dyn Write| execute_adoption(&inv, env(scratch.path(), out, &mut Vec::new()));
        let mut baseline = Vec::new();
        assert_eq!(run(&mut baseline), 1);
        let events: Vec<serde_json::Value> = String::from_utf8(baseline.clone())
            .expect("stdout")
            .lines()
            .map(|line| serde_json::from_str(line).expect("event"))
            .collect();
        assert_eq!(events[1]["code"], "status_pin_mismatch");
        assert_eq!(events[2]["exit_code"], 1);
        for remaining in [
            0,
            baseline
                .iter()
                .position(|b| *b == b'\n')
                .expect("first event")
                + 1,
            baseline.len() - 1,
        ] {
            assert_eq!(run(&mut Truncated::after_bytes(remaining)), 141);
        }
    }

    #[test]
    fn status_drift_and_hook_mutation_reports_propagate_broken_pipe() {
        let scratch = dx_test_scratch::scratch("adoption-live-pipe-");
        dx_adopt::write_version_pin(scratch.path(), "9.9.9").expect("drifted pin");
        let inv = invocation(&["status", "--output=json"]);
        let mut baseline = Vec::new();
        let run =
            |out: &mut dyn Write| execute_adoption(&inv, env(scratch.path(), out, &mut Vec::new()));
        assert_eq!(run(&mut baseline), 1);
        for remaining in baseline
            .iter()
            .enumerate()
            .filter(|(_, byte)| **byte == b'\n')
            .map(|(index, _)| index + 1)
            .filter(|offset| *offset < baseline.len())
        {
            assert_eq!(run(&mut Truncated::after_bytes(remaining)), 141);
        }
        for verb in ["install", "uninstall"] {
            let scratch = dx_test_scratch::scratch("hooks-live-pipe-");
            std::fs::create_dir(scratch.path().join(".git")).expect("git");
            if verb == "uninstall" {
                dx_adopt::install_hooks(scratch.path()).expect("install");
            }
            let inv = invocation(&["hooks", "--apply", verb]);
            assert_eq!(
                execute_adoption(
                    &inv,
                    env(
                        scratch.path(),
                        &mut Truncated::after_bytes(0),
                        &mut Vec::new()
                    )
                ),
                141
            );
        }
    }

    #[test]
    fn adoption_output_truncation_fails_at_each_document_boundary() {
        for words in [
            vec!["status"],
            vec!["status", "--output=json"],
            vec!["status", "--dry-run"],
            vec!["status", "--dry-run", "--output=json"],
            vec!["upgrade", "--from=1.0.0", "--to=2.0.0"],
            vec!["upgrade", "--from=1.0.0", "--to=2.0.0", "--output=json"],
            vec!["upgrade", "--from=1.0.0", "--to=2.0.0", "--dry-run"],
            vec![
                "upgrade",
                "--from=1.0.0",
                "--to=2.0.0",
                "--dry-run",
                "--output=json",
            ],
            vec!["hooks", "status"],
            vec!["hooks", "status", "--dry-run"],
            vec!["hooks", "install", "--dry-run"],
            vec!["hooks", "uninstall", "--dry-run"],
            vec!["hooks", "run", "pre-commit", "--dry-run"],
            vec!["completion", "bash"],
            vec!["completion", "bash", "--dry-run"],
            vec!["init", "--dry-run"],
            vec!["new", "rust", "demo", "--dry-run"],
        ] {
            let scratch = dx_test_scratch::scratch("adoption-pipe-");
            dx_adopt::write_version_pin(scratch.path(), "0.0.0").expect("pin");
            let inv = invocation(&words);
            let run = |out: &mut dyn Write| {
                execute_adoption(&inv, env(scratch.path(), out, &mut Vec::new()))
            };
            let mut baseline = Vec::new();
            let code = run(&mut baseline);
            assert!(code == 0 || words[0] == "upgrade", "{words:?}: {code}");
            assert!(!baseline.is_empty(), "{words:?}");
            let mut boundaries = vec![0, baseline.len() - 1];
            if words[0] != "completion" {
                boundaries.extend(
                    baseline
                        .iter()
                        .enumerate()
                        .filter(|(_, byte)| **byte == b'\n')
                        .map(|(index, _)| index + 1)
                        .filter(|offset| *offset < baseline.len()),
                );
            }
            boundaries.sort_unstable();
            boundaries.dedup();
            for remaining in boundaries {
                assert_eq!(
                    run(&mut Truncated::after_bytes(remaining)),
                    141,
                    "{words:?} after {remaining} bytes"
                );
            }
        }
    }

    #[test]
    fn quiet_suppresses_summaries_but_not_results() {
        let inv = invocation(&["init", "--dry-run", "--quiet", "demo"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-cmd-quiet-init-");
        let root = scratch.path().to_path_buf();
        let (code, out, _err) = run(&inv, &root);
        assert_eq!(code, 0);
        assert!(out.is_empty());

        let inv = invocation(&["status", "--quiet"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-cmd-quiet-status-");
        let root = scratch.path().to_path_buf();
        std::fs::create_dir_all(root.join(".dx")).expect("dx");
        std::fs::write(root.join(".dx/version"), "0.0.0\n").expect("pin");
        let (code, out, _err) = run(&inv, &root);
        assert_eq!(code, 0);
        assert!(!out.is_empty());

        let inv = invocation(&["version", "--dry-run", "--pin=0.0.0", "--quiet"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-cmd-quiet-version-dryrun-");
        let root = scratch.path().to_path_buf();
        let (code, out, _err) = run(&inv, &root);
        assert_eq!(code, 0);
        assert!(out.is_empty());

        let inv = invocation(&["version", "--quiet"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-cmd-quiet-version-");
        let root = scratch.path().to_path_buf();
        std::fs::create_dir_all(root.join(".dx")).expect("dx");
        std::fs::write(root.join(".dx/version"), "0.0.0\n").expect("pin");
        let (code, out, _err) = run(&inv, &root);
        assert_eq!(code, 0);
        assert!(!out.is_empty());
    }
}
