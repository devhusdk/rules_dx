use std::io::Write;

use crate::exec::common::{check_stdout_write, refusal};
use dx_plan::args::Invocation;

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
    let record = match dx_adopt::read_init_record(workspace) {
        Ok(record) => record,
        Err(error) => {
            return operational(invocation, out, err, CODE_INIT_FAILED, &error.to_string())
        }
    };
    let parts = record.parts.union(invocation.init_parts);
    let agents = parts.contains(dx_adopt::InitPart::Agents);
    if !invocation.applies() {
        return check_init(invocation, workspace, module, &record, parts, out, err);
    }
    let planned = match dx_adopt::plan_init_files_triples(
        module,
        parts,
        &invocation.init_rust_target_triples,
    ) {
        Ok(files) => files,
        Err(error) => return invalid_module_code(invocation, out, err, &error),
    };
    let mut destinations: Vec<&str> = planned.iter().map(|file| file.path.as_str()).collect();
    destinations.push(dx_adopt::INIT_RECORD_REL);
    if let Err(message) = super::wiring::preflight(workspace, "", agents, &destinations) {
        return operational(invocation, out, err, CODE_INIT_FAILED, &message);
    }
    let mut applied = dx_adopt::Applied::default();
    let mut notes = Vec::new();
    let written = dx_adopt::merge_init_record(workspace, invocation.init_parts, &mut applied)
        .and_then(|()| {
            dx_adopt::apply_init_triples(
                workspace,
                module,
                parts,
                &invocation.init_rust_target_triples,
                &mut applied,
            )
        });
    let result = match written {
        Ok(()) => {
            if applied.refused.iter().any(|path| path == ".bazelignore")
                && !std::fs::read_to_string(workspace.join(".bazelignore"))
                    .is_ok_and(|text| dx_adopt::bazelignore_covers_dx(&text))
            {
                notes.push(
                    ".bazelignore lacks .dx; add it so Bazel skips the IDE output base".to_owned(),
                );
            }
            super::wiring::apply(workspace, "", agents, &mut applied, &mut notes)
        }
        Err(error @ dx_adopt::AdoptError::InitInvalidModule { .. }) => {
            return invalid_module_code(invocation, out, err, &error);
        }
        Err(error) => Err(error.to_string()),
    };
    super::wiring::report(
        invocation,
        &applied,
        &notes,
        result,
        CODE_INIT_FAILED,
        out,
        err,
    )
}

fn check_init(
    invocation: &Invocation,
    workspace: &std::path::Path,
    module: &str,
    record: &dx_adopt::InitRecord,
    parts: dx_adopt::InitParts,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let files = match dx_adopt::plan_init_files_triples(
        module,
        parts,
        &invocation.init_rust_target_triples,
    ) {
        Ok(files) => files,
        Err(error) => return invalid_module_code(invocation, out, err, &error),
    };
    if !record.unknown.is_empty() {
        let _ = writeln!(
            err,
            "dx: {} names parts this dx does not know: {}",
            dx_adopt::INIT_RECORD_REL,
            record.unknown.join(", ")
        );
    }
    let wiring = super::wiring::drift(workspace, parts.contains(dx_adopt::InitPart::Agents));
    let mut missing: Vec<String> = files
        .iter()
        .map(|file| file.path.clone())
        .filter(|path| !workspace.join(path).exists())
        .collect();
    missing.extend(wiring.missing);
    let stale = wiring.stale;
    let other = wiring.other;
    let invalid_pin = dx_adopt::read_version_pin(workspace)
        .ok()
        .filter(|pin| !dx_adopt::pin_is_valid(pin));
    if missing.is_empty() && stale.is_empty() && other.is_empty() && invalid_pin.is_none() {
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
        .chain(other)
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
    use crate::adopt::test_support::{env, invocation, run, Truncated};
    use dx_process::pre_exec_code;

    #[test]
    fn init_check_names_missing_hooks_and_agents_block() {
        let scratch = dx_test_scratch::scratch("dx-adopt-init-check-wiring-");
        let root = scratch.path().to_path_buf();
        let apply = invocation(&["init", "--apply", "--with=agents"]);
        let check = invocation(&["init"]);
        assert_eq!(run(&apply, &root).0, 0);
        assert_eq!(run(&check, &root).0, 0);
        std::fs::remove_file(root.join(".claude/settings.json")).expect("remove");
        let (code, _out, err) = run(&check, &root);
        assert_eq!(code, 1);
        assert!(err.contains("missing .claude/settings.json"), "{err}");
        std::fs::write(root.join(".claude/settings.json"), "{}\n").expect("empty");
        let (code, _out, err) = run(&check, &root);
        assert_eq!(code, 1);
        assert!(err.contains(".claude/settings.json lacks hooks: "), "{err}");
        assert_eq!(run(&apply, &root).0, 0);
        std::fs::remove_file(root.join(".claude/settings.json")).expect("remove");
        assert_eq!(run(&apply, &root).0, 0);
        assert_eq!(run(&check, &root).0, 0);
        std::fs::write(root.join("AGENTS.md"), "# Mine\n").expect("no block");
        let (code, _out, err) = run(&check, &root);
        assert_eq!(code, 1);
        assert!(err.contains("stale AGENTS.md"), "{err}");
        assert_eq!(run(&apply, &root).0, 0);
        std::fs::write(root.join("CLAUDE.md"), "# Mine\n").expect("no import");
        let (code, _out, err) = run(&check, &root);
        assert_eq!(code, 1);
        assert!(err.contains("CLAUDE.md lacks @AGENTS.md"), "{err}");
        assert_eq!(run(&apply, &root).0, 0);
        let (code, out, _err) = run(&check, &root);
        assert_eq!(code, 0, "{out}");
    }

    #[test]
    fn init_apply_with_a_vscode_file_writes_nothing() {
        let scratch = dx_test_scratch::scratch("dx-adopt-init-vscode-file-");
        let root = scratch.path().to_path_buf();
        std::fs::write(root.join(".vscode"), "file\n").expect("blocker");
        let (code, out, err) = run(&invocation(&["init", "--apply", "--with=vscode"]), &root);
        assert_eq!(code, 1);
        assert!(out.is_empty(), "{out}");
        assert!(err.contains("init_failed"), "{err}");
        assert!(err.contains(".vscode is not a directory"), "{err}");
        for path in [".dx", "tools", ".claude", "AGENTS.md", "CLAUDE.md"] {
            assert!(!root.join(path).exists(), "{path}");
        }
    }

    #[test]
    fn init_apply_finishes_every_write_when_stdout_closes() {
        let scratch = dx_test_scratch::scratch("dx-adopt-init-closed-stdout-");
        let root = scratch.path().to_path_buf();
        let inv = invocation(&["init", "--apply", "--with=agents"]);
        let mut out = Truncated::after_lines(1);
        let mut err = Vec::new();
        let code = super::super::execute_adoption(&inv, env(&root, &mut out, &mut err));
        assert_eq!(code, 141);
        for path in [
            "tools/dx",
            ".claude/settings.json",
            "AGENTS.md",
            "CLAUDE.md",
        ] {
            assert!(root.join(path).exists(), "{path}");
        }
        assert_eq!(run(&invocation(&["init"]), &root).0, 0);
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
    fn init_full_applies_every_part() {
        let inv = invocation(&["init", "--apply", "--full"]);
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
            serde_json::json!(dx_worktree::setup::allow_rules())
        );
    }

    #[test]
    fn init_apply_appends_the_import_to_an_existing_claude_md() {
        let scratch = dx_test_scratch::scratch("dx-adopt-init-claude-");
        let root = scratch.path().to_path_buf();
        std::fs::write(root.join("CLAUDE.md"), "# Mine\n").expect("seed");
        let inv = invocation(&["init", "--apply", "--with=agents"]);
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
        let (code, _out, err) = run(&invocation(&["init", "--apply", "--with=agents"]), &root);
        assert_eq!(code, 0);
        assert_eq!(
            std::fs::read_to_string(root.join(".claude/settings.json")).expect("read"),
            own
        );
        for rule in dx_worktree::setup::allow_rules() {
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
        let inv = invocation(&["init", "--apply", "--with=agents"]);
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

    #[test]
    fn init_default_apply_writes_the_essentials_only() {
        let scratch = dx_test_scratch::scratch("dx-adopt-init-essentials-");
        let root = scratch.path().to_path_buf();
        let (code, _out, err) = run(&invocation(&["init", "--apply"]), &root);
        assert_eq!(code, 0, "{err}");
        for path in [
            ".dx/version",
            ".gitignore",
            ".bazelignore",
            "MODULE.bazel.snippet",
            "tools/dx",
        ] {
            assert!(root.join(path).exists(), "{path}");
        }
        for path in [
            ".envrc",
            ".devcontainer",
            ".vscode",
            ".github",
            ".claude",
            ".codex",
            ".gemini",
            "AGENTS.md",
            "CLAUDE.md",
            dx_adopt::INIT_RECORD_REL,
        ] {
            assert!(!root.join(path).exists(), "{path}");
        }
        assert_eq!(run(&invocation(&["init"]), &root).0, 0);
    }

    #[test]
    fn init_with_records_parts_and_check_follows_the_record() {
        let scratch = dx_test_scratch::scratch("dx-adopt-init-with-");
        let root = scratch.path().to_path_buf();
        let (code, out, err) = run(
            &invocation(&["init", "--apply", "--with=vscode,agents"]),
            &root,
        );
        assert_eq!(code, 0, "{err}");
        assert!(out.contains("wrote .dx/init.toml"), "{out}");
        for path in [
            ".vscode/settings.json",
            ".vscode/extensions.json",
            ".claude/settings.json",
            "AGENTS.md",
        ] {
            assert!(root.join(path).exists(), "{path}");
        }
        assert!(!root.join(".envrc").exists());
        let record = dx_adopt::read_init_record(&root).expect("record");
        assert_eq!(record.parts.names(), ["vscode", "agents"]);
        let check = invocation(&["init"]);
        assert_eq!(run(&check, &root).0, 0);
        std::fs::remove_file(root.join(".vscode/settings.json")).expect("remove");
        let (code, _out, err) = run(&check, &root);
        assert_eq!(code, 1);
        assert!(err.contains("missing .vscode/settings.json"), "{err}");
    }

    #[test]
    fn init_apply_merges_parts_into_the_record() {
        let scratch = dx_test_scratch::scratch("dx-adopt-init-merge-");
        let root = scratch.path().to_path_buf();
        assert_eq!(
            run(&invocation(&["init", "--apply", "--with=vscode"]), &root).0,
            0
        );
        assert_eq!(
            run(&invocation(&["init", "--apply", "--with=envrc"]), &root).0,
            0
        );
        assert_eq!(run(&invocation(&["init", "--apply"]), &root).0, 0);
        let record = dx_adopt::read_init_record(&root).expect("record");
        assert_eq!(record.parts.names(), ["envrc", "vscode"]);
    }

    #[test]
    fn init_check_ignores_parts_a_legacy_tree_never_recorded() {
        let scratch = dx_test_scratch::scratch("dx-adopt-init-legacy-");
        let root = scratch.path().to_path_buf();
        assert_eq!(run(&invocation(&["init", "--apply", "--full"]), &root).0, 0);
        std::fs::remove_file(root.join(dx_adopt::INIT_RECORD_REL)).expect("legacy");
        for path in [
            ".envrc",
            ".vscode/settings.json",
            "AGENTS.md",
            ".claude/settings.json",
        ] {
            std::fs::remove_file(root.join(path)).expect("remove");
        }
        let (code, out, err) = run(&invocation(&["init"]), &root);
        assert_eq!(code, 0, "{out}{err}");
        let (code, _out, err) = run(&invocation(&["init", "--with=github-ci"]), &root);
        assert_eq!(code, 0, "{err}");
        std::fs::remove_file(root.join(".github/workflows/ci.yml")).expect("remove");
        let (code, _out, err) = run(&invocation(&["init", "--with=github-ci"]), &root);
        assert_eq!(code, 1);
        assert!(err.contains("missing .github/workflows/ci.yml"), "{err}");
    }

    #[test]
    fn init_check_notes_unknown_recorded_parts_and_rejects_a_bad_record() {
        let scratch = dx_test_scratch::scratch("dx-adopt-init-unknown-");
        let root = scratch.path().to_path_buf();
        assert_eq!(run(&invocation(&["init", "--apply"]), &root).0, 0);
        std::fs::write(root.join(dx_adopt::INIT_RECORD_REL), "with = [\"later\"]\n")
            .expect("record");
        let (code, _out, err) = run(&invocation(&["init"]), &root);
        assert_eq!(code, 0, "{err}");
        assert!(err.contains("does not know: later"), "{err}");
        std::fs::write(root.join(dx_adopt::INIT_RECORD_REL), "with = [\n").expect("bad");
        let (code, _out, err) = run(&invocation(&["init", "--apply"]), &root);
        assert_eq!(code, 1);
        assert!(err.contains("init_failed"), "{err}");
    }
}
