use std::io::Write;

use crate::args::Invocation;
use crate::exec::common::{check_stdout_write, refusal};

use super::{operational, summaries_suppressed};

pub(crate) const CODE_INIT_FAILED: &str = "init_failed";

fn invalid_module_code(
    invocation: &Invocation,
    out: &mut dyn Write,
    err: &mut dyn Write,
    error: &dx_adopt::AdoptError,
) -> i32 {
    match error {
        dx_adopt::AdoptError::InitInvalidModule { .. } => {
            refusal(invocation.output, out, err, &error.to_string())
        }
        _ => operational(invocation, out, err, CODE_INIT_FAILED, &error.to_string()),
    }
}

pub(crate) fn execute_init(
    invocation: &Invocation,
    workspace: &std::path::Path,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let module = invocation
        .targets
        .first()
        .map_or("my_project", String::as_str);
    if !invocation.applies() {
        return check_init(invocation, workspace, module, out, err);
    }
    if let Err(error) = super::wiring::preflight(workspace) {
        return operational(invocation, out, err, CODE_INIT_FAILED, &error.to_string());
    }
    match dx_adopt::apply_init(workspace, module) {
        Ok(entries) => {
            for entry in entries {
                if entry == "---" {
                    continue;
                }
                if let Some(path) = entry.strip_prefix("refused:") {
                    let _ = writeln!(err, "dx: {path} (absent-only, left untouched)");
                    if path == ".bazelignore"
                        && !std::fs::read_to_string(workspace.join(path))
                            .is_ok_and(|text| dx_adopt::bazelignore_covers_dx(&text))
                    {
                        let _ = writeln!(
                            err,
                            "dx: .bazelignore lacks .dx; add it so Bazel skips the IDE output base"
                        );
                    }
                } else if !summaries_suppressed(invocation) {
                    if let Err(exit) = check_stdout_write(writeln!(out, "wrote {entry}")) {
                        return exit;
                    }
                }
            }
            match super::wiring::apply(invocation, workspace, "", CODE_INIT_FAILED, out, err) {
                Ok(()) => 0,
                Err(code) => code,
            }
        }
        Err(error) => invalid_module_code(invocation, out, err, &error),
    }
}

fn check_init(
    invocation: &Invocation,
    workspace: &std::path::Path,
    module: &str,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let files = match dx_adopt::plan_init_files(module) {
        Ok(files) => files,
        Err(error) => return invalid_module_code(invocation, out, err, &error),
    };
    let mut paths: Vec<&str> = files.iter().map(|file| file.path.as_str()).collect();
    paths.push(dx_adopt::LAUNCHER_SH_REL);
    paths.push(dx_adopt::LAUNCHER_CMD_REL);
    let missing: Vec<String> = paths
        .into_iter()
        .filter(|path| !workspace.join(path).exists())
        .map(str::to_owned)
        .collect();
    let stale: Vec<String> =
        dx_adopt::stale_launchers(workspace, &super::wiring::launcher_pin(workspace))
            .into_iter()
            .map(|(rel, _)| rel)
            .collect();
    let invalid_pin = dx_adopt::read_version_pin(workspace)
        .ok()
        .filter(|pin| !dx_adopt::pin_is_valid(pin));
    if missing.is_empty() && stale.is_empty() && invalid_pin.is_none() {
        if !summaries_suppressed(invocation) {
            if let Err(exit) = check_stdout_write(writeln!(out, "init current: {module}")) {
                return exit;
            }
        }
        return 0;
    }
    let rewrite = dx_adopt::pin_rewrite_hint(dx_adopt::MODULE_VERSION);
    let mut clauses: Vec<String> = [("missing", missing), ("stale", stale)]
        .into_iter()
        .filter(|(_, paths)| !paths.is_empty())
        .map(|(kind, paths)| format!("{kind} {}", paths.join(", ")))
        .collect();
    let has_entries = !clauses.is_empty();
    if let Some(pin) = &invalid_pin {
        clauses.push(format!("invalid .dx/version ({pin:?})"));
    }
    let fix = match (has_entries, invalid_pin.is_some()) {
        (true, true) => format!("run `dx init {module} --apply`, then {rewrite}"),
        (false, true) => rewrite,
        _ => format!("run `dx init {module} --apply`"),
    };
    operational(
        invocation,
        out,
        err,
        CODE_INIT_FAILED,
        &format!("init drift: {} ({fix})", clauses.join("; ")),
    )
}

#[cfg(test)]
mod tests {
    use crate::adopt::test_support::{invocation, run};
    use dx_process::pre_exec_code;

    #[test]
    fn a_non_semver_pin_fails_init_before_any_write() {
        let scratch = dx_test_scratch::scratch("dx-adopt-init-semver-");
        let root = scratch.path().to_path_buf();
        std::fs::create_dir_all(root.join(".dx")).expect("dx");
        std::fs::write(root.join(".dx/version"), "v1\n").expect("pin");
        let (code, _out, err) = run(&invocation(&["init", "--apply"]), &root);
        assert_eq!(code, 1);
        assert!(err.contains("init_failed"), "{err}");
        assert!(!root.join(".gitignore").exists());
        assert!(!root.join("tools").exists());
    }

    #[test]
    fn init_check_reports_a_stale_launcher() {
        let scratch = dx_test_scratch::scratch("dx-adopt-init-check-stale-");
        let root = scratch.path().to_path_buf();
        let (code, _out, _err) = run(&invocation(&["init", "--apply"]), &root);
        assert_eq!(code, 0);
        std::fs::write(root.join(".dx/version"), "4.5.6\n").expect("pin");
        let (code, _out, err) = run(&invocation(&["init"]), &root);
        assert_eq!(code, 1);
        assert!(
            err.contains("init drift: stale tools/dx, tools/dx.cmd"),
            "{err}"
        );
        let (code, _out, _err) = run(&invocation(&["init", "--apply"]), &root);
        assert_eq!(code, 0);
        let (code, out, _err) = run(&invocation(&["init"]), &root);
        assert_eq!(code, 0, "{out}");
    }

    #[test]
    fn init_check_reports_an_invalid_pin_as_drift() {
        for bad in ["", "v1"] {
            let scratch = dx_test_scratch::scratch("dx-adopt-init-check-pin-");
            let root = scratch.path().to_path_buf();
            let (code, _out, _err) = run(&invocation(&["init", "--apply"]), &root);
            assert_eq!(code, 0);
            std::fs::write(root.join(".dx/version"), format!("{bad}\n")).expect("pin");
            let (code, _out, err) = run(&invocation(&["init"]), &root);
            assert_eq!(code, 1, "{bad:?}");
            assert!(
                err.contains("init drift: ") && err.contains("invalid .dx/version"),
                "{err}"
            );
            assert!(
                err.contains(&format!(
                    "write {} to .dx/version",
                    dx_adopt::MODULE_VERSION
                )),
                "{err}"
            );
            dx_adopt::write_version_pin(&root, dx_adopt::DX_VERSION).expect("fix");
            let (code, out, _err) = run(&invocation(&["init"]), &root);
            assert_eq!(code, 0, "{out}");
            assert!(out.contains("init current"), "{out}");
        }
    }

    #[test]
    fn init_applies_absent_only() {
        let inv = invocation(&["init", "--apply"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-init-apply-");
        let root = scratch.path().to_path_buf();
        let (code, _out, _err) = run(&inv, &root);
        assert_eq!(code, 0);
        assert!(root.join(".dx/version").exists());
        assert!(root.join(".devcontainer/devcontainer.json").exists());
        assert!(root.join(".claude/settings.json").exists());
        assert!(root.join(".codex/hooks.json").exists());
        assert!(root.join(".gemini/settings.json").exists());
        let agents = std::fs::read_to_string(root.join("AGENTS.md")).expect("agents");
        assert!(
            agents.contains("tools/dx fix --apply --affected"),
            "{agents}"
        );
        assert!(agents.contains("tools/dx test --affected"), "{agents}");
        assert_eq!(
            std::fs::read_to_string(root.join("CLAUDE.md")).expect("claude"),
            "@AGENTS.md\n"
        );
        let settings: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(root.join(".claude/settings.json")).expect("settings"),
        )
        .expect("json");
        assert_eq!(
            settings["permissions"]["allow"],
            serde_json::json!(crate::worktree::setup::allow_rules())
        );
    }

    #[test]
    fn init_apply_appends_the_import_to_an_existing_claude_md() {
        let scratch = dx_test_scratch::scratch("dx-adopt-init-claude-");
        let root = scratch.path().to_path_buf();
        std::fs::write(root.join("CLAUDE.md"), "# Mine\n").expect("seed");
        let inv = invocation(&["init", "--apply"]);
        let (code, out, _err) = run(&inv, &root);
        assert_eq!(code, 0);
        assert!(out.contains("wrote CLAUDE.md"), "{out}");
        assert_eq!(
            std::fs::read_to_string(root.join("CLAUDE.md")).expect("claude"),
            "# Mine\n\n@AGENTS.md\n"
        );
        let (code, out, _err) = run(&inv, &root);
        assert_eq!(code, 0);
        assert!(!out.contains("CLAUDE.md"), "{out}");
    }

    #[test]
    fn init_apply_lists_missing_allow_entries_without_touching_settings() {
        let scratch = dx_test_scratch::scratch("dx-adopt-init-allow-");
        let root = scratch.path().to_path_buf();
        std::fs::create_dir_all(root.join(".claude")).expect("dir");
        let own = "{\"hooks\": {}}\n";
        std::fs::write(root.join(".claude/settings.json"), own).expect("seed");
        let (code, _out, err) = run(&invocation(&["init", "--apply"]), &root);
        assert_eq!(code, 0);
        assert_eq!(
            std::fs::read_to_string(root.join(".claude/settings.json")).expect("read"),
            own
        );
        for rule in crate::worktree::setup::allow_rules() {
            assert!(err.contains(&rule), "{rule}: {err}");
        }
    }

    #[test]
    fn init_notes_an_existing_bazelignore_without_dx() {
        let inv = invocation(&["init", "--apply"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-init-bazelignore-");
        let root = scratch.path().to_path_buf();
        std::fs::write(root.join(".bazelignore"), "node_modules\n").expect("seed");
        let (code, _out, err) = run(&inv, &root);
        assert_eq!(code, 0, "{err}");
        assert!(
            err.contains("dx: .bazelignore lacks .dx; add it so Bazel skips the IDE output base"),
            "{err}"
        );
        assert_eq!(
            std::fs::read_to_string(root.join(".bazelignore")).expect("read"),
            "node_modules\n"
        );
        std::fs::write(root.join(".bazelignore"), "node_modules\n.dx/\n").expect("seed");
        let (code, _out, err) = run(&inv, &root);
        assert_eq!(code, 0, "{err}");
        assert!(!err.contains("lacks .dx"), "{err}");
        assert_eq!(
            std::fs::read_to_string(root.join(".bazelignore")).expect("read"),
            "node_modules\n.dx/\n"
        );
    }

    #[test]
    fn init_apply_adds_the_block_to_a_hand_written_file_and_keeps_the_rest() {
        let scratch = dx_test_scratch::scratch("dx-adopt-init-agents-");
        let root = scratch.path().to_path_buf();
        std::fs::write(root.join("AGENTS.md"), "# Mine\n\nKeep this.\n").expect("seed");
        let inv = invocation(&["init", "--apply"]);
        let (code, out, _err) = run(&inv, &root);
        assert_eq!(code, 0);
        assert!(out.contains("wrote AGENTS.md"), "{out}");
        let agents = std::fs::read_to_string(root.join("AGENTS.md")).expect("agents");
        assert!(agents.starts_with("# Mine\n\nKeep this.\n"), "{agents}");
        assert!(
            agents.contains("tools/dx worktree add <branch>"),
            "{agents}"
        );
        let (code, out, _err) = run(&inv, &root);
        assert_eq!(code, 0);
        assert!(!out.contains("AGENTS.md"), "{out}");
        assert_eq!(
            std::fs::read_to_string(root.join("AGENTS.md")).expect("agents"),
            agents
        );
    }

    #[test]
    fn init_default_checks_without_writing_and_apply_writes() {
        let scratch = dx_test_scratch::scratch("dx-adopt-init-check-");
        let root = scratch.path().to_path_buf();
        let inv = invocation(&["init"]);
        let (code, _out, err) = run(&inv, &root);
        assert_eq!(code, 1);
        assert!(err.contains("init_failed"), "{err}");
        assert!(err.contains("init drift"), "{err}");
        assert!(err.contains("--apply"), "{err}");
        assert!(!root.join(".dx/version").exists(), "check must not write");
        assert!(!root.join(".devcontainer/devcontainer.json").exists());
        let inv = invocation(&["init", "--apply"]);
        let (code, _out, _err) = run(&inv, &root);
        assert_eq!(code, 0);
        assert!(root.join(".dx/version").exists());
        let inv = invocation(&["init"]);
        let (code, out, _err) = run(&inv, &root);
        assert_eq!(code, 0, "{out}");
        assert!(out.contains("current"), "{out}");
    }

    #[test]
    fn init_rejects_invalid_modules_pre_exec_without_writing() {
        for (module, quiet) in ["Bad Name", "../evil", "with\nnewline", "semi;colon"]
            .into_iter()
            .flat_map(|module| [(module, false), (module, true)])
        {
            let scratch = dx_test_scratch::scratch("dx-adopt-init-reject-");
            let root = scratch.path().to_path_buf();
            std::fs::write(root.join("sentinel"), "stay").expect("sentinel");
            let mut words = vec!["init", module];
            if quiet {
                words.push("--quiet");
            }
            let inv = invocation(&words);
            let (code, _out, err) = run(&inv, &root);
            assert_eq!(code, pre_exec_code(), "{module:?}");
            assert!(err.contains("invalid module name"), "{err}");
            let mut entries: Vec<String> = Vec::new();
            for entry in std::fs::read_dir(&root).expect("read") {
                entries.push(
                    entry
                        .expect("entry")
                        .file_name()
                        .to_string_lossy()
                        .into_owned(),
                );
            }
            assert_eq!(entries, vec!["sentinel".to_owned()], "{module:?}");
        }
    }
}
