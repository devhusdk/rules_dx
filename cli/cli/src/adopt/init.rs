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
    if let Err(error) = dx_adopt::plan_launcher_files(workspace, &launcher_pin(workspace), true) {
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
            match write_launcher(invocation, workspace, out, err) {
                Ok(()) => {}
                Err(code) => return code,
            }
            match crate::worktree::setup::apply(workspace) {
                Ok(outcome) => {
                    for note in &outcome.notes {
                        let _ = writeln!(err, "{note}");
                    }
                    if !summaries_suppressed(invocation) {
                        for entry in &outcome.wrote {
                            if let Err(exit) = check_stdout_write(writeln!(out, "wrote {entry}")) {
                                return exit;
                            }
                        }
                    }
                }
                Err(error) => {
                    return operational(invocation, out, err, CODE_INIT_FAILED, &error.to_string());
                }
            }
            match super::agents::write(workspace, true) {
                Ok(changed) => {
                    if changed && !summaries_suppressed(invocation) {
                        if let Err(exit) =
                            check_stdout_write(writeln!(out, "wrote {}", super::agents::FILE))
                        {
                            return exit;
                        }
                    }
                    0
                }
                Err(error) => {
                    operational(invocation, out, err, CODE_INIT_FAILED, &error.to_string())
                }
            }
        }
        Err(error) => invalid_module_code(invocation, out, err, &error),
    }
}

fn launcher_pin(workspace: &std::path::Path) -> String {
    match dx_adopt::read_version_pin(workspace) {
        Ok(pin) if !pin.is_empty() => pin,
        _ => dx_adopt::DX_VERSION.to_owned(),
    }
}

fn write_launcher(
    invocation: &Invocation,
    workspace: &std::path::Path,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<(), i32> {
    match dx_adopt::write_launcher_files(workspace, &launcher_pin(workspace)) {
        Ok(entries) => {
            for entry in entries {
                if let Some(path) = entry.strip_prefix("refused:") {
                    let _ = writeln!(err, "dx: {path} (absent-only, left untouched)");
                } else if !summaries_suppressed(invocation) {
                    check_stdout_write(writeln!(out, "wrote {entry}"))?;
                }
            }
            Ok(())
        }
        Err(error) => Err(operational(
            invocation,
            out,
            err,
            CODE_INIT_FAILED,
            &error.to_string(),
        )),
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
    let stale: Vec<String> = dx_adopt::stale_launchers(workspace, &launcher_pin(workspace))
        .into_iter()
        .map(|(rel, _)| rel)
        .collect();
    if missing.is_empty() && stale.is_empty() {
        if !summaries_suppressed(invocation) {
            if let Err(exit) = check_stdout_write(writeln!(out, "init current: {module}")) {
                return exit;
            }
        }
        return 0;
    }
    operational(
        invocation,
        out,
        err,
        CODE_INIT_FAILED,
        &format!(
            "init drift: {} (run `dx init {module} --apply`)",
            [("missing", missing), ("stale", stale)]
                .into_iter()
                .filter(|(_, paths)| !paths.is_empty())
                .map(|(kind, paths)| format!("{kind} {}", paths.join(", ")))
                .collect::<Vec<_>>()
                .join("; ")
        ),
    )
}

#[cfg(test)]
mod tests {
    use crate::adopt::test_support::{invocation, run};
    use dx_process::pre_exec_code;

    #[test]
    fn launcher_is_written_when_absent_or_stale() {
        let inv = invocation(&["init", "--apply"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-init-launcher-");
        let root = scratch.path();
        let (mut out, mut err) = (Vec::new(), Vec::new());
        super::write_launcher(&inv, root, &mut out, &mut err).expect("writes");
        assert_eq!(
            String::from_utf8_lossy(&out),
            "wrote tools/dx\nwrote tools/dx.cmd\n"
        );
        assert_eq!(
            dx_adopt::launcher_version(root, "tools/dx").as_deref(),
            Some(dx_adopt::DX_VERSION)
        );
        let (mut out, mut err) = (Vec::new(), Vec::new());
        super::write_launcher(&inv, root, &mut out, &mut err).expect("keeps");
        assert!(out.is_empty());
        assert!(String::from_utf8_lossy(&err).contains("tools/dx (absent-only, left untouched)"));
        std::fs::write(root.join("tools/dx"), dx_adopt::render_launcher_sh("1.2.3"))
            .expect("stale");
        std::fs::write(root.join("tools/dx.cmd"), "@echo off\r\nmine\r\n").expect("custom");
        std::fs::create_dir_all(root.join(".dx")).expect("dx");
        std::fs::write(root.join(".dx/version"), "4.5.6\n").expect("pin");
        let (mut out, mut err) = (Vec::new(), Vec::new());
        super::write_launcher(&inv, root, &mut out, &mut err).expect("syncs");
        assert_eq!(String::from_utf8_lossy(&out), "wrote tools/dx\n");
        assert!(
            String::from_utf8_lossy(&err).contains("tools/dx.cmd (absent-only, left untouched)")
        );
        assert_eq!(
            dx_adopt::launcher_version(root, "tools/dx").as_deref(),
            Some("4.5.6")
        );
        assert_eq!(
            std::fs::read_to_string(root.join("tools/dx.cmd")).expect("cmd"),
            "@echo off\r\nmine\r\n"
        );
    }

    #[test]
    fn a_non_semver_pin_is_an_init_failure() {
        let inv = invocation(&["init", "--apply"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-init-launcher-semver-");
        let root = scratch.path();
        std::fs::create_dir_all(root.join(".dx")).expect("dx");
        std::fs::write(root.join(".dx/version"), "1\"; x\n").expect("pin");
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = super::write_launcher(&inv, root, &mut out, &mut err);
        assert_eq!(code, Err(1));
        assert!(String::from_utf8_lossy(&err).contains("init_failed"));
        assert!(!root.join("tools").exists());
    }

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
    fn launcher_write_failure_is_an_init_failure() {
        let inv = invocation(&["init", "--apply"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-init-launcher-fail-");
        std::fs::write(scratch.path().join("tools"), "file").expect("blocker");
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = super::write_launcher(&inv, scratch.path(), &mut out, &mut err);
        assert_eq!(code, Err(1));
        assert!(String::from_utf8_lossy(&err).contains("init_failed"));
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
        let agents = std::fs::read_to_string(root.join("AGENTS.md")).expect("agents");
        assert!(
            agents.contains("tools/dx fix --apply --affected"),
            "{agents}"
        );
        assert!(agents.contains("tools/dx test --affected"), "{agents}");
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
