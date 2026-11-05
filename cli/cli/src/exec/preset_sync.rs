//! Writes the generated Bazel preset layers before a command runs.

use std::io::Write;
use std::path::Path;

use dx_plan::args::{Command, Invocation};

/// Writes missing or stale imported layers; a failure prints one warning and changes nothing else.
pub fn sync_preset_layers(invocation: &Invocation, workspace: &Path, err: &mut dyn Write) {
    if matches!(
        invocation.command,
        Command::Init | Command::Status | Command::Upgrade
    ) || !workspace.join("MODULE.bazel").is_file()
    {
        return;
    }
    let legacy = dx_adopt::inspect_preset(workspace).is_ok_and(|state| state.legacy_correctness);
    if let Err(error) = dx_adopt::sync_preset(workspace) {
        let _ = writeln!(err, "dx: warning: preset layers not refreshed: {error}");
    } else if legacy {
        let _ = writeln!(err, "dx: {}", dx_adopt::WERROR_NOTE);
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
        let imports: String = dx_adopt::preset_fragment::Layer::ALL
            .iter()
            .map(|layer| {
                format!(
                    "{}\n",
                    dx_adopt::preset_fragment::import_line(".dx/bazelrc", *layer)
                )
            })
            .collect();
        std::fs::write(root.join(".bazelrc"), imports).expect("rc");
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
    fn missing_layers_are_written_quietly() {
        let scratch = workspace(true);
        let root = scratch.path();
        assert_eq!(run("build", root), "");
        assert!(root.join(LAYER).is_file());
    }

    #[test]
    fn a_layer_without_the_line_prints_the_note_once() {
        let scratch = workspace(true);
        let root = scratch.path();
        run("build", root);
        let path = root.join(LAYER);
        let text = std::fs::read_to_string(&path).expect("layer");
        let legacy: String = text
            .lines()
            .filter(|line| !line.contains("warnings_as_errors"))
            .map(|line| format!("{line}\n"))
            .collect();
        std::fs::write(&path, legacy).expect("legacy");
        let err = run("build", root);
        assert!(err.contains(dx_adopt::WERROR_NOTE), "{err}");
        assert_eq!(run("build", root), "");
    }

    #[test]
    fn init_status_and_upgrade_do_not_write() {
        for command in ["init", "status", "upgrade"] {
            let scratch = workspace(true);
            run(command, scratch.path());
            assert!(!scratch.path().join(LAYER).exists(), "{command}");
        }
    }

    #[test]
    fn no_module_does_not_write() {
        let scratch = workspace(false);
        run("build", scratch.path());
        assert!(!scratch.path().join(LAYER).exists());
    }

    #[test]
    fn a_failure_warns_once() {
        let scratch = workspace(true);
        let root = scratch.path();
        std::fs::write(root.join(".dx"), "file").expect("blocker");
        let err = run("build", root);
        assert!(
            err.starts_with("dx: warning: preset layers not refreshed: "),
            "{err}"
        );
        assert_eq!(err.lines().count(), 1);
    }
}
