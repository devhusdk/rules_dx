//! Brings `.dx/local` up to date after a command that built something, when the
//! sync fingerprint says the generated files or their inputs changed.

use std::io::Write;

use dx_output::OutputMode;
use dx_plan::args::{Command, Invocation};

use super::common::{ActionTally, Env};
use super::notice_codes::CODE_SYNC_FAILED;
use super::sync_fingerprint::{self, Check, Record};

/// Runs after `invocation` exited 0; never changes its exit code.
pub(crate) fn after_success(invocation: &Invocation, env: Env<'_>) {
    let builds = matches!(
        invocation.command,
        Command::Build | Command::Test | Command::Coverage | Command::Run | Command::Check
    );
    if !builds || env.ci || invocation.quiet || invocation.here {
        return;
    }
    let workspace = env.workspace;
    if !dx_path::state::local_dir(workspace)
        .join(dx_path::state::SETUPS)
        .join(dx_path::state::CURRENT)
        .exists()
    {
        return;
    }
    let record = match sync_fingerprint::check(workspace) {
        Check::Same => return,
        Check::SameFailed(message) => {
            warn(env.err, &message);
            return;
        }
        Check::Missing => Record {
            targets: Vec::new(),
            artifacts: Vec::new(),
            sources: Vec::new(),
        },
        Check::Changed(record) => record,
    };
    let mut sync = invocation.clone();
    sync.command = Command::Sync;
    sync.targets = record.targets.clone();
    sync.reports.clear();
    sync.affected = false;
    sync.affected_handoff = None;
    sync.quiet = true;
    sync.verbose = false;
    sync.output = OutputMode::Text { quiet: true };
    sync.timings = false;
    let actions = ActionTally::default();
    let mut captured = Vec::new();
    let code = super::managed::execute_managed(
        &sync,
        Env {
            out: &mut std::io::sink(),
            err: &mut captured,
            nonce: env.nonce.wrapping_add(1),
            actions: &actions,
            ..env
        },
    );
    if code == 0 {
        let _ = writeln!(env.err, "dx: synced .dx/local (codegen changed)");
        return;
    }
    let captured = String::from_utf8_lossy(&captured);
    let message = captured
        .lines()
        .rev()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map_or_else(
            || format!("sync build exited {code}"),
            |line| line.strip_prefix("dx: ").unwrap_or(line).to_owned(),
        );
    let _ = sync_fingerprint::record(workspace, &record, Some(&message));
    warn(env.err, &message);
}

fn warn(err: &mut dyn Write, message: &str) {
    let _ = writeln!(
        err,
        "dx: warning: {CODE_SYNC_FAILED}: {message}; next: dx sync"
    );
}

#[cfg(test)]
mod tests {
    use super::super::test_support::*;
    use super::*;

    fn is_sync(argv: &[String]) -> bool {
        argv.iter()
            .any(|arg| arg.contains(dx_codegen::OUTPUT_GROUP))
    }

    fn syncs(harness: &Harness) -> usize {
        harness
            .build_argv
            .borrow()
            .iter()
            .filter(|argv| is_sync(argv))
            .count()
    }

    fn synced(name: &str) -> Harness {
        let harness = Harness::new(name);
        let (code, out, err) = harness.run(&["sync"]);
        assert_eq!(code, 0, "{out}{err}");
        harness.build_argv.borrow_mut().clear();
        harness
    }

    #[test]
    fn a_build_with_nothing_changed_makes_one_bazel_call() {
        let harness = synced("auto-sync-same");
        let (code, out, err) = harness.run(&["build"]);
        assert_eq!(code, 0, "{out}{err}");
        assert_eq!(harness.build_argv.borrow().len(), 1);
        assert!(!err.contains("synced"), "{err}");
    }

    #[test]
    fn a_rebuilt_artifact_syncs_once_after_the_build() {
        let harness = synced("auto-sync-changed");
        let artifact = harness.temp.join("gen.rs");
        std::fs::write(&artifact, "pub fn a() {}\n").expect("artifact");
        let record = Record {
            targets: Vec::new(),
            artifacts: vec![artifact.clone()],
            sources: Vec::new(),
        };
        sync_fingerprint::record(&harness.workspace, &record, None).expect("record");
        std::fs::write(&artifact, "pub fn a() {}\npub fn b() {}\n").expect("rebuild");

        let (code, out, err) = harness.run(&["build"]);
        assert_eq!(code, 0, "{out}{err}");
        let argv = harness.build_argv.borrow().clone();
        assert_eq!(argv.len(), 2, "{argv:?}");
        assert!(!is_sync(&argv[0]) && is_sync(&argv[1]), "{argv:?}");
        assert!(
            err.contains("dx: synced .dx/local (codegen changed)"),
            "{err}"
        );

        harness.build_argv.borrow_mut().clear();
        let (code, _, err) = harness.run(&["build"]);
        assert_eq!(code, 0, "{err}");
        assert_eq!(harness.build_argv.borrow().len(), 1);
    }

    #[test]
    fn json_output_keeps_the_sync_off_stdout() {
        let harness = Harness::new("auto-sync-json");
        commit_clean_pair(&harness, 'a', 'b');
        let (code, out, err) = harness.run(&["build", "--output=json"]);
        assert_eq!(code, 0, "{out}{err}");
        assert_eq!(syncs(&harness), 1);
        assert!(err.contains("dx: synced .dx/local"), "{err}");
        let last = out.lines().last().expect("events");
        assert!(last.contains("command_finished"), "{out}");
        assert_eq!(out.matches("command_started").count(), 1, "{out}");
    }

    #[test]
    fn a_failed_sync_warns_without_failing_the_command() {
        let mut harness = Harness::new("auto-sync-fails");
        commit_clean_pair(&harness, 'a', 'b');
        harness.launch_codes = vec![0, 1];
        let (code, out, err) = harness.run(&["build"]);
        assert_eq!(code, 0, "{out}{err}");
        assert_eq!(syncs(&harness), 1);
        assert_eq!(err.matches("sync_failed").count(), 1, "{err}");
        assert!(err.contains("; next: dx sync"), "{err}");

        harness.build_argv.borrow_mut().clear();
        harness.launch_codes = vec![0];
        let (code, out, err) = harness.run(&["build"]);
        assert_eq!(code, 0, "{out}{err}");
        assert_eq!(harness.build_argv.borrow().len(), 1);
        assert_eq!(err.matches("sync_failed").count(), 1, "{err}");
    }

    #[test]
    fn ci_quiet_quality_and_unsynced_workspaces_never_sync() {
        let harness = Harness::new("auto-sync-ci");
        commit_clean_pair(&harness, 'a', 'b');
        let (code, _, err) = harness.run_with_ci(&["build"], true);
        assert_eq!(code, 0, "{err}");
        let (code, _, err) = harness.run(&["build", "--quiet"]);
        assert_eq!(code, 0, "{err}");
        harness.run(&["lint"]);
        assert_eq!(syncs(&harness), 0);

        let fresh = Harness::new("auto-sync-no-current");
        let (code, _, err) = fresh.run(&["build"]);
        assert_eq!(code, 0, "{err}");
        assert_eq!(fresh.build_argv.borrow().len(), 1);
    }
}
