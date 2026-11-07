//! Brings `.dx` in step with `dx.toml` before a command runs.

use std::io::Write;
use std::path::Path;

use dx_generate::Mode;
use dx_plan::args::{Command, Invocation};

/// Syncs `.dx`; a failure prints one warning and changes nothing else.
pub fn sync_preset_layers(invocation: &Invocation, workspace: &Path, err: &mut dyn Write) {
    if matches!(
        invocation.command,
        Command::Init | Command::Status | Command::Upgrade
    ) || !workspace.join("MODULE.bazel").is_file()
        || keeps_repo_preset(workspace)
    {
        return;
    }
    let mode = match invocation.command {
        Command::Generate | Command::Check => Mode::Full,
        _ => Mode::Fast,
    };
    if !dx_path::state::fingerprint_path(workspace).exists() {
        match dx_adopt::legacy_layout::migrate(workspace) {
            Ok(lines) => {
                for line in lines {
                    let _ = writeln!(err, "{line}");
                }
            }
            Err(error) => {
                let _ = writeln!(err, "dx: warning: old .dx layout not migrated: {error}");
            }
        }
    }
    if let Err(error) = dx_generate::sync(workspace, mode, &mut dx_adopt::detected_families) {
        let _ = writeln!(err, "dx: warning: .dx not refreshed: {error}");
    }
}

/// Whether the root `.bazelrc` imports the checked-in preset of rules_dx itself.
fn keeps_repo_preset(workspace: &Path) -> bool {
    let text = std::fs::read_to_string(workspace.join(".bazelrc")).unwrap_or_default();
    let prefix = format!("{}/", dx_adopt::preset_fragment::REPO_DIR);
    dx_adopt::preset_fragment::bazelrc_imports(&text)
        .iter()
        .any(|rel| rel.starts_with(&prefix))
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
    fn the_rules_dx_repo_layout_does_not_write() {
        let scratch = workspace(true);
        let root = scratch.path();
        let import = dx_adopt::preset_fragment::import_line(
            dx_adopt::preset_fragment::REPO_DIR,
            dx_adopt::preset_fragment::Layer::Correctness,
        );
        std::fs::write(root.join(".bazelrc"), format!("{import}\n")).expect("rc");
        run("build", root);
        assert!(!root.join(".dx").exists());
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
    fn the_first_sync_migrates_an_old_layout() {
        let scratch = workspace(true);
        let root = scratch.path();
        dx_test_scratch::write(root, "pkg/a.py", "");
        dx_test_scratch::write(root, ".dx/config/ruff/standard.toml", "old\n");
        dx_test_scratch::write(root, ".dx/config/ruff/strict.toml", "old\n");
        dx_test_scratch::write(
            root,
            "ruff.toml",
            "extend = \".dx/config/ruff/strict.toml\"\n",
        );
        let err = run("build", root);
        assert_eq!(
            err,
            "dx: migrated ruff.toml: .dx/config/ruff/strict.toml -> .dx/config/ruff/base.toml\n\
             dx: migrated dx.toml: tier = \"strict\"\n"
        );
        let ruff = dx_generate::tools_gen::TOOL_CONFIGS
            .iter()
            .find(|config| config.id == "ruff")
            .expect("ruff");
        let base = std::fs::read_to_string(root.join(".dx/config/ruff/base.toml")).expect("base");
        assert_eq!(base, ruff.strict);
        assert!(!root.join(".dx/config/ruff/strict.toml").exists());
        assert_eq!(run("build", root), "");
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
