//! Brings `.dx` in step with `dx.toml` before a command runs.

use std::io::Write;
use std::path::Path;
use std::sync::Mutex;

use dx_generate::Mode;
use dx_plan::args::{Command, Invocation};

static REWRITTEN: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// The `.dx` files the sync in this invocation wrote or deleted.
pub fn rewritten() -> Vec<String> {
    REWRITTEN
        .lock()
        .map(|list| list.clone())
        .unwrap_or_default()
}

/// Syncs `.dx`; a failure prints one warning and changes nothing else.
pub fn sync_preset_layers(invocation: &Invocation, workspace: &Path, err: &mut dyn Write) {
    if matches!(
        invocation.command,
        Command::Init | Command::Status | Command::Upgrade
    ) || !workspace.join("MODULE.bazel").is_file()
    {
        return;
    }
    let mode = match invocation.command {
        Command::Generate | Command::Check => Mode::Full,
        Command::Security
        | Command::License
        | Command::Lint
        | Command::Typecheck
        | Command::Format
        | Command::Build
        | Command::Test
        | Command::Coverage
        | Command::Bench
        | Command::Run
        | Command::Deploy
        | Command::Clean
        | Command::Update
        | Command::Sync
        | Command::Init
        | Command::Upgrade
        | Command::Status
        | Command::Watch
        | Command::Completion
        | Command::Docs
        | Command::Worktree
        | Command::Fuzz
        | Command::Config => Mode::Fast,
    };
    match dx_generate::sync(workspace, mode, &mut dx_adopt::detected_families) {
        Ok(report) => {
            if let Ok(mut list) = REWRITTEN.lock() {
                list.extend(report.written.iter().chain(&report.deleted).cloned());
            }
            for warning in report.warnings {
                writeln!(err, "{warning}").ok();
            }
        }
        Err(error) => {
            writeln!(err, "dx: warning: .dx not refreshed: {error}").ok();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LAYER: &str = ".dx/bazelrc/correctness.bazelrc";

    fn workspace(module: bool) -> dx_test_scratch::TempDir {
        let scratch = dx_test_scratch::scratch("dx-preset-sync-");
        let root = scratch.path();
        if module {
            std::fs::write(root.join("MODULE.bazel"), "").expect("module");
        }
        std::fs::write(root.join("dx.toml"), "schema_version = 1\n").expect("dx.toml");
        scratch
    }

    fn run(command: &str, root: &Path) -> String {
        let argv: &[&str] = if command == "upgrade" {
            &["upgrade", "--to", "1.0.0"]
        } else {
            &[command]
        };
        let invocation = dx_plan::args::parse(argv).expect("parse");
        let mut err = Vec::new();
        sync_preset_layers(&invocation, root, &mut err);
        String::from_utf8(err).expect("utf8")
    }

    #[test]
    fn a_fast_run_writes_the_layers_quietly() {
        let scratch = workspace(true);
        let root = scratch.path();
        assert_eq!(run("build", root), "");
        assert!(root.join(LAYER).is_file());
        assert!(root.join(".dx/BUILD.bazel").is_file());
    }

    #[test]
    fn init_status_and_upgrade_do_not_write() {
        for command in ["init", "status", "upgrade"] {
            let scratch = workspace(true);
            run(command, scratch.path());
            assert!(!scratch.path().join(".dx").exists(), "{command}");
        }
    }

    #[test]
    fn no_module_does_not_write() {
        let scratch = workspace(false);
        run("build", scratch.path());
        assert!(!scratch.path().join(".dx").exists());
    }

    #[test]
    fn generate_wires_a_language_added_after_the_first_sync() {
        let scratch = workspace(true);
        let root = scratch.path();
        assert_eq!(run("build", root), "");
        let policy = || std::fs::read_to_string(root.join(".dx/BUILD.bazel")).expect("policy");
        assert!(!policy().contains("\"python\""), "{}", policy());
        dx_test_scratch::write(root, "pkg/a.py", "");
        assert_eq!(run("generate", root), "");
        assert!(policy().contains("\"python\""), "{}", policy());
    }

    #[test]
    fn a_failure_warns_once() {
        let scratch = workspace(true);
        let root = scratch.path();
        std::fs::write(root.join(".dx"), "file").expect("blocker");
        let err = run("build", root);
        assert!(err.starts_with("dx: warning: .dx not refreshed: "), "{err}");
        assert_eq!(err.lines().count(), 1);
    }
}
