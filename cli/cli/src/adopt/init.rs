use std::io::Write;

use crate::exec::common::{check_stdout_write, emit_built, emit_event, refusal};
use crate::exec::notice_codes::{CODE_INIT_ABSENT, CODE_INIT_PLAN_PARTIAL};
use dx_output::{
    command_finished, command_started, init_plan_event, notice_event, FinishedCounts, NoticeEvent,
    OutputMode, PlanFindingEvent,
};
use dx_plan::args::{Agent, Invocation, ProfileArg};

use super::{operational, summaries_suppressed};

pub(crate) const CODE_INIT_FAILED: &str = "init_failed";

fn invalid_module_code(
    invocation: &Invocation,
    out: &mut dyn Write,
    err: &mut dyn Write,
    error: &dx_adopt::AdoptError,
) -> i32 {
    match error {
        dx_adopt::AdoptError::InitInvalidModule { .. }
        | dx_adopt::AdoptError::InitInvalidDir { .. } => {
            refusal(invocation.output, out, err, &error.to_string())
        }
        _ => operational(invocation, out, err, CODE_INIT_FAILED, &error.to_string()),
    }
}

/// The directory one `dx init` run works in.
struct Target {
    /// The directory as given, `/`-separated, or empty for the current directory.
    dir: String,
    /// Where the files go.
    root: std::path::PathBuf,
    /// What reported paths start with: `<dir>/`, or nothing.
    prefix: String,
    module: String,
    /// The entry that makes the directory an existing tree; `None` when it is fresh.
    blocker: Option<String>,
    agents: Vec<Agent>,
    profile: Option<dx_adopt::Profile>,
}

impl Target {
    fn resolve(
        invocation: &Invocation,
        workspace: &std::path::Path,
    ) -> Result<Self, dx_adopt::AdoptError> {
        let given = invocation.targets.first().map_or("", String::as_str);
        let dir = dx_adopt::validate_init_dir(given)?;
        let module = dx_adopt::init_dir_module(&dir);
        if module.is_empty() {
            return Err(dx_adopt::AdoptError::InitInvalidModule {
                module: given.to_owned(),
                reason: "the directory name holds no [a-z0-9] character to name the module"
                    .to_owned(),
            });
        }
        let root = if dir.is_empty() {
            workspace.to_path_buf()
        } else {
            workspace.join(&dir)
        };
        let prefix = if dir.is_empty() {
            String::new()
        } else {
            format!("{dir}/")
        };
        let blocker = dx_adopt::fresh_blocker(&root);
        Ok(Self {
            dir,
            root,
            prefix,
            module,
            blocker,
            agents: invocation.agents.clone(),
            profile: invocation.profile.map(|profile| match profile {
                ProfileArg::Standard => dx_adopt::Profile::Standard,
                ProfileArg::Strict => dx_adopt::Profile::Strict,
            }),
        })
    }

    /// The files this run plans, relative to `root`.
    fn plan(&self) -> Result<Vec<super::init_files::InitFile>, dx_adopt::AdoptError> {
        super::init_files::plan_with_profile(
            &self.root,
            &self.module,
            self.blocker.is_none(),
            &self.agents,
            self.profile,
        )
    }

    /// The command that fixes drift in this directory.
    fn apply_command(&self) -> String {
        let mut command = "dx init".to_owned();
        if !self.dir.is_empty() {
            command.push(' ');
            command.push_str(&self.dir);
        }
        command.push_str(" --apply");
        if let Some(profile) = self.profile {
            command.push_str(&format!(" --profile={}", profile.name()));
        }
        if !self.agents.is_empty() {
            let names: Vec<&str> = self.agents.iter().map(|agent| agent.name()).collect();
            command.push_str(&format!(" --agents={}", names.join(",")));
        }
        command
    }
}

fn prefixed(prefix: &str, paths: Vec<String>) -> impl Iterator<Item = String> + '_ {
    paths.into_iter().map(move |path| format!("{prefix}{path}"))
}

pub(crate) fn execute_init(
    invocation: &Invocation,
    workspace: &std::path::Path,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let target = match Target::resolve(invocation, workspace) {
        Ok(target) => target,
        Err(error) => return invalid_module_code(invocation, out, err, &error),
    };
    let root = target.root.as_path();
    if invocation.applies() && invocation.output == OutputMode::Json {
        return refusal(
            invocation.output,
            out,
            err,
            "dx init --apply has no JSON output; drop --output=json",
        );
    }
    if !invocation.applies() {
        return check_init(invocation, &target, out, err);
    }
    let planned = match target.plan() {
        Ok(files) => files,
        Err(error) => return invalid_module_code(invocation, out, err, &error),
    };
    if let Err(message) = super::init_files::preflight(root, &target.prefix, &planned) {
        return operational(invocation, out, err, CODE_INIT_FAILED, &message);
    }
    let mut notes = Vec::new();
    if let Some(profile) = target.profile {
        notes.extend(
            dx_adopt::profile_notes(root, profile)
                .into_iter()
                .map(|note| format!("{}{note}", target.prefix)),
        );
    }
    notes.extend(
        dx_adopt::kept_config_notes(root)
            .into_iter()
            .map(|note| format!("{}{note}", target.prefix)),
    );
    if let Some(blocker) = &target.blocker {
        if !root.join("MODULE.bazel").exists() {
            notes.push(format!(
                "{}{blocker} exists, treating this as an existing repo; create MODULE.bazel first or run in an empty directory",
                target.prefix
            ));
        }
    }
    if let Some(plan) = dx_adopt::with_overrides_include(&dx_adopt::render_consumer_module_setup())
        .ok()
        .map(|setup| dx_adopt::preflight(root, &setup))
        .filter(|plan| plan.applicable)
    {
        let hard: Vec<&str> = plan.hard().map(|f| f.subject.as_str()).collect();
        if !hard.is_empty() {
            let _ = write!(
                err,
                "{}",
                dx_adopt::render_plan_text(&target.module, &target.apply_command(), &plan)
            );
            return operational(
                invocation,
                out,
                err,
                CODE_INIT_FAILED,
                &format!(
                    "init plan: {} hard conflicts ({}); next: resolve {} hard conflict(s), then run `{}`",
                    hard.len(),
                    hard.join(", "),
                    hard.len(),
                    target.apply_command()
                ),
            );
        }
        notes.extend(
            plan.findings
                .iter()
                .map(|f| format!("{} ({}:{})", f.hint, f.file, f.line)),
        );
    }
    let mut applied = dx_adopt::Applied::default();
    let result = super::init_files::apply(
        root,
        &target.prefix,
        &planned,
        super::init_files::Mode::Create,
        &mut applied,
        &mut notes,
    );
    if result.is_ok() {
        notes.push(
            "next: run `tools/dx env --apply` to fill .dx/local, then `direnv allow`".to_owned(),
        );
    }
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

fn absent_message(absent: &[String], apply: &str) -> String {
    format!(
        "init absent: {} (run `{apply}` to recreate)",
        absent.join(", ")
    )
}

fn check_init(
    invocation: &Invocation,
    target: &Target,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let workspace = target.root.as_path();
    let module = target.module.as_str();
    let files = match target.plan() {
        Ok(files) => files,
        Err(error) => return invalid_module_code(invocation, out, err, &error),
    };
    let wiring = super::init_files::drift(workspace, &files);
    let missing: Vec<String> = prefixed(&target.prefix, wiring.missing).collect();
    let stale: Vec<String> = prefixed(&target.prefix, wiring.stale).collect();
    let absent: Vec<String> = prefixed(&target.prefix, wiring.absent).collect();
    let other: Vec<String> = prefixed(&target.prefix, wiring.other).collect();
    let invalid_pin = dx_adopt::read_version_pin(workspace)
        .ok()
        .filter(|pin| !dx_adopt::pin_is_valid(pin));
    let plan = dx_adopt::with_overrides_include(&dx_adopt::render_consumer_module_setup())
        .ok()
        .map(|setup| dx_adopt::preflight(workspace, &setup))
        .filter(|plan| plan.applicable);
    let json = invocation.output == OutputMode::Json;
    if json {
        if let Err(exit) = emit_built(out, command_started(invocation.command.name(), "default")) {
            return exit;
        }
    }
    if let (Some(plan), true) = (&plan, json) {
        for f in &plan.findings {
            let event = init_plan_event(&PlanFindingEvent {
                kind: f.kind.id().to_owned(),
                subject: f.subject.clone(),
                before: f.before.clone(),
                after: f.after.clone(),
                file: f.file.clone(),
                line: f.line,
                hint: f.hint.clone(),
                severity: if f.hard { "hard" } else { "advisory" }.to_owned(),
            });
            if let Err(exit) = emit_built(out, event) {
                return exit;
            }
        }
        for note in &plan.notes {
            if let Err(exit) = emit_built(
                out,
                notice_event(&NoticeEvent::info(CODE_INIT_PLAN_PARTIAL, note.clone())),
            ) {
                return exit;
            }
        }
    } else if let Some(plan) = &plan {
        if !summaries_suppressed(invocation) {
            if let Err(exit) = check_stdout_write(write!(
                out,
                "{}",
                dx_adopt::render_plan_text(module, &target.apply_command(), plan)
            )) {
                return exit;
            }
        }
    }
    let conflicts: Vec<String> = plan
        .iter()
        .flat_map(|plan| plan.hard().map(|f| f.subject.clone()))
        .collect();
    if missing.is_empty()
        && stale.is_empty()
        && other.is_empty()
        && invalid_pin.is_none()
        && conflicts.is_empty()
    {
        let apply = target.apply_command();
        if json {
            if !absent.is_empty() {
                let notice = NoticeEvent::info(CODE_INIT_ABSENT, absent_message(&absent, &apply));
                if let Err(exit) = emit_built(out, notice_event(&notice)) {
                    return exit;
                }
            }
            if let Err(exit) = emit_event(out, &command_finished(0, &FinishedCounts::default())) {
                return exit;
            }
        } else if !absent.is_empty() {
            let _ = writeln!(err, "{}", absent_message(&absent, &apply));
        } else if !summaries_suppressed(invocation) {
            if let Err(exit) = check_stdout_write(writeln!(out, "init current: {module}")) {
                return exit;
            }
        }
        return 0;
    }
    let rewrite = dx_adopt::pin_rewrite_hint();
    let mut clauses: Vec<String> = [("missing", missing), ("stale", stale), ("absent", absent)]
        .into_iter()
        .filter(|(_, paths)| !paths.is_empty())
        .map(|(kind, paths)| format!("{kind} {}", paths.join(", ")))
        .chain(other)
        .collect();
    let has_entries = !clauses.is_empty();
    if let Some(pin) = &invalid_pin {
        clauses.push(format!("invalid .dx/pins ({pin:?})"));
    }
    if !conflicts.is_empty() {
        clauses.push(format!(
            "init plan: {} hard conflicts ({})",
            conflicts.len(),
            conflicts.join(", ")
        ));
    }
    let apply = target.apply_command();
    let fix = match (has_entries, invalid_pin.is_some()) {
        (true, true) => format!("run `{apply}`, then {rewrite}"),
        (false, true) => rewrite,
        (false, false) => "resolve the conflicts in the init plan above".to_owned(),
        _ => format!("run `{apply}`"),
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
        let apply = invocation(&["init", "--apply", "--agents=claude"]);
        let check = invocation(&["init"]);
        assert_eq!(run(&apply, &root).0, 0);
        assert_eq!(run(&check, &root).0, 0);
        std::fs::write(root.join(".claude/settings.json"), "{}\n").expect("empty");
        let (code, _out, err) = run(&check, &root);
        assert_eq!(code, 1);
        assert!(
            err.contains("stale .claude/settings.json (lacks hooks: "),
            "{err}"
        );
        assert_eq!(run(&apply, &root).0, 0);
        std::fs::write(root.join("AGENTS.md"), "# Mine\n").expect("no block");
        let (code, _out, err) = run(&check, &root);
        assert_eq!(code, 1);
        assert!(err.contains("stale AGENTS.md"), "{err}");
        assert_eq!(run(&apply, &root).0, 0);
        std::fs::write(root.join("CLAUDE.md"), "# Mine\n").expect("no import");
        let (code, _out, err) = run(&check, &root);
        assert_eq!(code, 1);
        assert!(err.contains("stale CLAUDE.md (lacks @AGENTS.md)"), "{err}");
        assert_eq!(run(&apply, &root).0, 0);
        let (code, out, _err) = run(&check, &root);
        assert_eq!(code, 0, "{out}");
    }

    #[test]
    fn default_apply_writes_no_tool_files_and_check_is_clean() {
        let scratch = dx_test_scratch::scratch("dx-adopt-init-no-tools-");
        let root = scratch.path().to_path_buf();
        assert_eq!(run(&invocation(&["init", "--apply"]), &root).0, 0);
        assert!(root.join("AGENTS.md").exists());
        for path in ["CLAUDE.md", ".claude", ".codex", ".gemini"] {
            assert!(!root.join(path).exists(), "{path}");
        }
        assert_eq!(run(&invocation(&["init"]), &root).0, 0);
    }

    #[test]
    fn agents_on_an_existing_repo_adds_only_that_tools_files() {
        let scratch = dx_test_scratch::scratch("dx-adopt-init-add-agent-");
        let root = scratch.path().to_path_buf();
        assert_eq!(run(&invocation(&["init", "--apply"]), &root).0, 0);
        let add = invocation(&["init", "--apply", "--agents=claude,claude"]);
        let (code, out, err) = run(&add, &root);
        assert_eq!(code, 0, "{err}");
        let wrote: Vec<&str> = out
            .lines()
            .filter(|line| line.starts_with("wrote "))
            .collect();
        assert_eq!(
            wrote,
            vec!["wrote .claude/settings.json", "wrote CLAUDE.md"],
            "{out}"
        );
        assert!(!root.join(".codex").exists() && !root.join(".gemini").exists());
        let (code, out, _err) = run(&add, &root);
        assert_eq!(code, 0);
        assert!(!out.contains("wrote "), "{out}");
    }

    #[test]
    fn a_deleted_tool_file_is_not_drift_and_is_not_recreated() {
        let scratch = dx_test_scratch::scratch("dx-adopt-init-deleted-tool-");
        let root = scratch.path().to_path_buf();
        let apply = invocation(&["init", "--apply", "--agents=claude"]);
        assert_eq!(run(&apply, &root).0, 0);
        std::fs::remove_file(root.join(".claude/settings.json")).expect("remove");
        assert_eq!(run(&invocation(&["init"]), &root).0, 0);
        assert_eq!(run(&invocation(&["init", "--apply"]), &root).0, 0);
        assert!(!root.join(".claude/settings.json").exists());
    }

    #[test]
    fn a_deleted_agents_md_is_named_as_absent_and_check_exits_zero() {
        let scratch = dx_test_scratch::scratch("dx-adopt-init-absent-");
        let root = scratch.path().to_path_buf();
        assert_eq!(run(&invocation(&["init", "--apply"]), &root).0, 0);
        std::fs::remove_file(root.join("AGENTS.md")).expect("remove");
        let (code, out, err) = run(&invocation(&["init"]), &root);
        assert_eq!(code, 0, "{out}{err}");
        assert!(
            err.contains("init absent: AGENTS.md (run `dx init --apply` to recreate)"),
            "{err}"
        );
        assert!(!out.contains("init current"), "{out}");
        let (code, out, err) = run(&invocation(&["init", "--output=json"]), &root);
        assert_eq!(code, 0, "{out}{err}");
        assert!(
            events(&out).iter().any(|e| {
                e["event"] == "notice"
                    && e["code"] == "init_absent"
                    && e["message"]
                        .as_str()
                        .is_some_and(|m| m.contains("AGENTS.md"))
            }),
            "{out}"
        );
        assert_eq!(run(&invocation(&["init", "--apply"]), &root).0, 0);
        assert!(root.join("AGENTS.md").exists());
        let (code, out, _err) = run(&invocation(&["init"]), &root);
        assert_eq!(code, 0, "{out}");
        assert!(out.contains("init current"), "{out}");
    }

    #[test]
    fn a_named_agent_with_a_missing_file_is_drift_and_the_hint_keeps_the_flag() {
        let scratch = dx_test_scratch::scratch("dx-adopt-init-named-agent-");
        let root = scratch.path().to_path_buf();
        assert_eq!(run(&invocation(&["init", "--apply"]), &root).0, 0);
        let (code, _out, err) = run(&invocation(&["init", "--agents=codex"]), &root);
        assert_eq!(code, 1);
        assert!(err.contains("missing .codex/hooks.json"), "{err}");
        assert!(err.contains("dx init --apply --agents=codex"), "{err}");
    }

    #[test]
    fn init_apply_with_a_hard_plan_conflict_writes_nothing() {
        let scratch = dx_test_scratch::scratch("dx-adopt-init-hard-plan-");
        let root = scratch.path().to_path_buf();
        let module = "module(name = \"app\")\nsingle_version_override(module_name = \"rules_rust\", version = \"0.1.0\")\n";
        std::fs::write(root.join("MODULE.bazel"), module).expect("module");
        let (code, out, err) = run(&invocation(&["init", "--apply"]), &root);
        assert_eq!(code, 1, "{err}");
        assert!(out.is_empty(), "{out}");
        assert!(err.contains("hard conflict"), "{err}");
        assert_eq!(
            std::fs::read_to_string(root.join("MODULE.bazel")).expect("read"),
            module
        );
        assert!(!root.join("AGENTS.md").exists());
        assert_eq!(run(&invocation(&["init"]), &root).0, 1);
    }

    #[test]
    fn init_apply_with_a_vscode_file_writes_nothing() {
        let scratch = dx_test_scratch::scratch("dx-adopt-init-vscode-file-");
        let root = scratch.path().to_path_buf();
        std::fs::write(root.join(".vscode"), "file\n").expect("blocker");
        let (code, out, err) = run(&invocation(&["init", "--apply"]), &root);
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
        let inv = invocation(&["init", "--apply"]);
        let mut out = Truncated::after_lines(1);
        let mut err = Vec::new();
        let code = super::super::execute_adoption(&inv, env(&root, &mut out, &mut err));
        assert_eq!(code, 141);
        for path in ["tools/dx", "AGENTS.md"] {
            assert!(root.join(path).exists(), "{path}");
        }
        assert_eq!(run(&invocation(&["init"]), &root).0, 0);
    }

    #[test]
    fn a_non_semver_pin_fails_init_before_any_write() {
        let scratch = dx_test_scratch::scratch("dx-adopt-init-semver-");
        let root = scratch.path().to_path_buf();
        std::fs::create_dir_all(root.join(".dx")).expect("dx");
        std::fs::write(root.join(".dx/pins"), "v1\n").expect("pin");
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
        std::fs::write(
            root.join("tools/dx"),
            "#!/bin/sh\nset -eu\nDX_PINS=.dx/pins\n",
        )
        .expect("legacy sh");
        std::fs::write(
            root.join("tools/dx.cmd"),
            "@echo off\r\nsetlocal\r\nset \"DX_PINS=.dx\\pins\"\r\n",
        )
        .expect("legacy cmd");
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
            std::fs::write(root.join(".dx/pins"), format!("{bad}\n")).expect("pin");
            let (code, _out, err) = run(&invocation(&["init"]), &root);
            assert_eq!(code, 1, "{bad:?}");
            assert!(
                err.contains("init drift: ") && err.contains("invalid .dx/pins"),
                "{err}"
            );
            assert!(err.contains("dx init --apply"), "{err}");
            dx_adopt::write_version_pin(&root, dx_adopt::DX_VERSION).expect("fix");
            let (code, out, _err) = run(&invocation(&["init"]), &root);
            assert_eq!(code, 0, "{out}");
            assert!(out.contains("init current"), "{out}");
        }
    }

    #[test]
    fn init_full_applies_every_part() {
        let inv = invocation(&["init", "--apply", "--agents=claude,codex,gemini,copilot"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-init-apply-");
        let root = scratch.path().to_path_buf();
        let (code, _out, _err) = run(&inv, &root);
        assert_eq!(code, 0);
        assert!(root.join(".dx/pins").exists());
        assert!(root.join(".devcontainer/devcontainer.json").exists());
        assert!(root.join(".claude/settings.json").exists());
        assert!(root.join(".codex/hooks.json").exists());
        assert!(root.join(".gemini/settings.json").exists());
        let agents = std::fs::read_to_string(root.join("AGENTS.md")).expect("agents");
        assert!(
            agents.contains("tools/dx check --apply --affected"),
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
    fn init_apply_merges_hooks_and_allow_rules_into_existing_settings() {
        let scratch = dx_test_scratch::scratch("dx-adopt-init-allow-");
        let root = scratch.path().to_path_buf();
        std::fs::create_dir_all(root.join(".claude")).expect("dir");
        std::fs::write(root.join(".claude/settings.json"), "{\"hooks\": {}}\n").expect("seed");
        let (code, out, _err) = run(&invocation(&["init", "--apply"]), &root);
        assert_eq!(code, 0);
        assert!(out.contains("wrote .claude/settings.json"), "{out}");
        let text = std::fs::read_to_string(root.join(".claude/settings.json")).expect("read");
        for rule in dx_worktree::setup::allow_rules() {
            assert!(text.contains(&rule), "{rule}: {text}");
        }
    }

    #[test]
    fn init_check_reports_a_bazelignore_without_dx_as_stale() {
        let scratch = dx_test_scratch::scratch("dx-adopt-init-bazelignore-stale-");
        let root = scratch.path().to_path_buf();
        assert_eq!(run(&invocation(&["init", "--apply"]), &root).0, 0);
        let check = invocation(&["init"]);
        assert_eq!(run(&check, &root).0, 0);
        std::fs::write(root.join(".bazelignore"), "node_modules\n").expect("edit");
        let (code, _out, err) = run(&check, &root);
        assert_eq!(code, 1);
        assert!(err.contains("stale .bazelignore"), "{err}");
        assert!(err.contains("run `dx init --apply`"), "{err}");
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
            err.contains(
                "dx: .bazelignore lacks .dx/local; add the dx lines so Bazel skips dx state"
            ),
            "{err}"
        );
        assert_eq!(
            std::fs::read_to_string(root.join(".bazelignore")).expect("read"),
            "node_modules\n"
        );
        std::fs::write(root.join(".bazelignore"), "node_modules\n.dx/local\n").expect("seed");
        let (code, _out, err) = run(&inv, &root);
        assert_eq!(code, 0, "{err}");
        assert!(!err.contains("lacks .dx"), "{err}");
        assert_eq!(
            std::fs::read_to_string(root.join(".bazelignore")).expect("read"),
            "node_modules\n.dx/local\n"
        );
    }

    #[test]
    fn init_migrates_a_bare_dx_bazelignore_line() {
        let scratch = dx_test_scratch::scratch("dx-adopt-init-bazelignore-bare-");
        let root = scratch.path().to_path_buf();
        assert_eq!(run(&invocation(&["init", "--apply"]), &root).0, 0);
        std::fs::write(root.join(".bazelignore"), "node_modules\n.dx\n").expect("edit");
        let (code, _out, err) = run(&invocation(&["init"]), &root);
        assert_eq!(code, 1);
        assert!(err.contains("stale .bazelignore"), "{err}");
        assert_eq!(run(&invocation(&["init", "--apply"]), &root).0, 0);
        let text = std::fs::read_to_string(root.join(".bazelignore")).expect("read");
        assert_eq!(
            text,
            format!(
                "node_modules\n{}",
                dx_adopt::scaffold::bazelignore_content()
            )
        );
        assert_eq!(run(&invocation(&["init"]), &root).0, 0);
    }

    #[test]
    fn init_notes_a_gitignore_that_hides_dx_config() {
        let scratch = dx_test_scratch::scratch("dx-adopt-init-gitignore-config-");
        let root = scratch.path().to_path_buf();
        let git = std::process::Command::new("git")
            .arg("-C")
            .arg(&root)
            .args(["init", "-q"])
            .status()
            .expect("git");
        assert!(git.success());
        std::fs::write(root.join(".gitignore"), ".dx/\n").expect("seed");
        let (code, _out, err) = run(&invocation(&["init", "--apply"]), &root);
        assert_eq!(code, 0, "{err}");
        assert!(
            err.contains(
                ".gitignore hides .dx/config; ignore /.dx/* instead of .dx/ and add !/.dx/config/"
            ),
            "{err}"
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
    fn init_check_prints_the_plan_and_fails_on_a_hard_conflict() {
        let scratch = dx_test_scratch::scratch("dx-adopt-init-plan-");
        let root = scratch.path().to_path_buf();
        let (code, _out, _err) = run(&invocation(&["init", "--apply"]), &root);
        assert_eq!(code, 0);
        let module = format!(
            "bazel_dep(name = \"rules_dx\", version = \"{}\")\nbazel_dep(name = \"rules_rust\", version = \"0.60.0\")\n",
            dx_adopt::DX_VERSION
        );
        std::fs::write(root.join("MODULE.bazel"), &module).expect("module");
        let (code, out, err) = run(&invocation(&["init"]), &root);
        assert_eq!(code, 0, "{out}{err}");
        assert!(out.contains("rules_rust 0.60.0 -> 0.75.0"), "{out}");
        assert!(out.contains("MODULE.bazel:2"), "{out}");
        let conflict = format!(
            "{module}single_version_override(module_name = \"rules_rust\", version = \"0.75.0\")\n"
        );
        std::fs::write(root.join("MODULE.bazel"), conflict).expect("module");
        let (code, out, err) = run(&invocation(&["init"]), &root);
        assert_eq!(code, 1, "{out}{err}");
        assert!(out.contains("override conflicts:"), "{out}");
        assert!(out.contains("MODULE.bazel:3"), "{out}");
        assert!(
            err.contains("init plan: 1 hard conflicts (rules_rust)"),
            "{err}"
        );
    }

    fn adopted_root(name: &str, extra: &str) -> std::path::PathBuf {
        let scratch = dx_test_scratch::scratch(name);
        let root = scratch.path().to_path_buf();
        std::mem::forget(scratch);
        let (code, _out, _err) = run(&invocation(&["init", "--apply"]), &root);
        assert_eq!(code, 0);
        let module = format!(
            "bazel_dep(name = \"rules_dx\", version = \"{}\")\nbazel_dep(name = \"rules_rust\", version = \"0.60.0\")\n{extra}",
            dx_adopt::DX_VERSION
        );
        std::fs::write(root.join("MODULE.bazel"), module).expect("module");
        root
    }

    fn events(out: &str) -> Vec<serde_json::Value> {
        out.lines()
            .map(|line| serde_json::from_str(line).expect("ndjson"))
            .collect()
    }

    #[test]
    fn init_check_json_emits_init_plan_events() {
        let root = adopted_root("dx-adopt-init-plan-json-", "");
        let (code, out, err) = run(&invocation(&["init", "--output=json"]), &root);
        assert_eq!(code, 0, "{out}{err}");
        let all = events(&out);
        let plan: Vec<_> = all.iter().filter(|e| e["event"] == "init_plan").collect();
        assert_eq!(plan.len(), 1, "{out}");
        let f = plan[0];
        assert_eq!(f["kind"], "version_raise");
        assert_eq!(f["subject"], "rules_rust");
        assert_eq!(f["before"], "0.60.0");
        assert_eq!(f["after"], "0.75.0");
        assert_eq!(f["path"], "MODULE.bazel");
        assert_eq!(f["line"], 2);
        assert_eq!(f["severity"], "advisory");
        assert!(f["hint"].as_str().is_some_and(|h| !h.is_empty()));
        assert!(
            all.iter()
                .any(|e| e["event"] == "notice" && e["code"] == "init_plan_partial"),
            "{out}"
        );
    }

    #[test]
    fn init_check_json_reports_a_hard_conflict_and_fails() {
        let root = adopted_root(
            "dx-adopt-init-plan-hard-",
            "single_version_override(module_name = \"rules_rust\", version = \"0.75.0\")\n",
        );
        let (code, out, err) = run(&invocation(&["init", "--output=json"]), &root);
        assert_eq!(code, 1, "{out}{err}");
        assert!(
            events(&out)
                .iter()
                .any(|e| e["event"] == "init_plan" && e["severity"] == "hard"),
            "{out}"
        );
    }

    #[test]
    fn init_check_reads_transitive_raises_from_the_lock() {
        let root = adopted_root("dx-adopt-init-plan-lock-", "");
        let lock = serde_json::json!({
            "lockFileVersion": 24,
            "registryFileHashes": {
                "https://bcr.bazel.build/modules/rules_rust/0.75.0/source.json": "x",
                "https://bcr.bazel.build/modules/rules_rust/0.60.0/source.json": "x",
                "https://bcr.bazel.build/modules/bazel_skylib/1.0.0/source.json": "x"
            }
        });
        std::fs::write(root.join("MODULE.bazel.lock"), lock.to_string()).expect("lock");
        let (code, out, err) = run(&invocation(&["init", "--output=json"]), &root);
        assert_eq!(code, 0, "{out}{err}");
        let all = events(&out);
        let subjects: Vec<_> = all
            .iter()
            .filter(|e| e["event"] == "init_plan")
            .map(|e| e["subject"].as_str().unwrap_or_default().to_owned())
            .collect();
        assert_eq!(subjects, vec!["bazel_skylib".to_owned()], "{out}");
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
        assert!(!root.join(".dx/pins").exists(), "check must not write");
        assert!(!root.join(".devcontainer/devcontainer.json").exists());
        let inv = invocation(&["init", "--apply"]);
        let (code, _out, _err) = run(&inv, &root);
        assert_eq!(code, 0);
        assert!(root.join(".dx/pins").exists());
        let inv = invocation(&["init"]);
        let (code, out, _err) = run(&inv, &root);
        assert_eq!(code, 0, "{out}");
        assert!(out.contains("current"), "{out}");
    }

    #[test]
    fn init_rejects_invalid_dirs_pre_exec_without_writing() {
        for (module, quiet) in ["../evil", "with\nnewline", "/abs", "a/../b", "+++"]
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
            assert!(
                err.contains("invalid directory for dx init")
                    || err.contains("invalid module name"),
                "{err}"
            );
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
    fn init_dir_creates_a_fresh_workspace_and_rechecks_clean() {
        let scratch = dx_test_scratch::scratch("dx-adopt-init-dir-");
        let root = scratch.path().to_path_buf();
        let (code, _out, err) = run(&invocation(&["init", "nested/Demo App"]), &root);
        assert_eq!(code, 1, "{err}");
        assert!(
            err.contains("missing nested/Demo App/")
                && err.contains(" nested/Demo App/MODULE.bazel"),
            "{err}"
        );
        assert!(err.contains("dx init nested/Demo App --apply"), "{err}");
        assert!(!root.join("nested").exists(), "check must not write");
        let (code, out, err) = run(&invocation(&["init", "--apply", "nested/Demo App"]), &root);
        assert_eq!(code, 0, "{err}");
        assert!(
            out.contains("wrote nested/Demo App/MODULE.bazel\n"),
            "{out}"
        );
        assert!(out.contains("wrote nested/Demo App/.dx/pins\n"), "{out}");
        let dir = root.join("nested/Demo App");
        let module = std::fs::read_to_string(dir.join("MODULE.bazel")).expect("module");
        assert!(module.contains("name = \"demo_app\""), "{module}");
        assert!(dir.join(".bazelversion").is_file());
        assert!(dir.join("tools/dx").is_file());
        assert!(!root.join(".dx").exists(), "the parent stays untouched");
        let (code, out, err) = run(&invocation(&["init", "nested/Demo App"]), &root);
        assert_eq!(code, 0, "{err}");
        assert!(out.contains("init current: demo_app"), "{out}");
    }

    #[test]
    fn init_dir_reapply_keeps_edited_files() {
        let scratch = dx_test_scratch::scratch("dx-adopt-init-reapply-");
        let root = scratch.path().to_path_buf();
        let (code, _out, err) = run(&invocation(&["init", "--apply", "demo"]), &root);
        assert_eq!(code, 0, "{err}");
        let module = root.join("demo/MODULE.bazel");
        let source = root.join("demo/hello.c");
        let edited_module = format!(
            "{}# edited\n",
            std::fs::read_to_string(&module).expect("module")
        );
        std::fs::write(&module, &edited_module).expect("edit module");
        std::fs::write(&source, "/* edited */\n").expect("source");
        let (code, _out, err) = run(&invocation(&["init", "--apply", "demo"]), &root);
        assert_eq!(code, 0, "{err}");
        assert_eq!(
            std::fs::read_to_string(&module).expect("module"),
            edited_module
        );
        assert_eq!(
            std::fs::read_to_string(&source).expect("source"),
            "/* edited */\n"
        );
    }

    #[test]
    fn init_retry_after_an_interrupted_fresh_init_writes_bazelversion() {
        let scratch = dx_test_scratch::scratch("dx-adopt-init-retry-");
        let root = scratch.path().to_path_buf();
        std::fs::write(
            root.join("MODULE.bazel"),
            dx_adopt::plan_fresh_files(dx_adopt::DEFAULT_INIT_MODULE)[0]
                .content
                .clone(),
        )
        .expect("module");
        let (code, out, err) = run(&invocation(&["init", "--apply"]), &root);
        assert_eq!(code, 0, "{err}");
        assert!(out.contains("wrote .bazelversion\n"), "{out}");
        assert!(root.join(".bazelversion").is_file());
    }

    #[test]
    fn init_in_a_non_empty_tree_writes_no_module() {
        let scratch = dx_test_scratch::scratch("dx-adopt-init-existing-");
        let root = scratch.path().to_path_buf();
        std::fs::create_dir(root.join("src")).expect("src");
        let (code, out, err) = run(&invocation(&["init", "--apply"]), &root);
        assert_eq!(code, 0, "{err}");
        assert!(
            err.contains("dx: src/ exists, treating this as an existing repo; create MODULE.bazel first or run in an empty directory\n"),
            "{err}"
        );
        assert!(!out.contains("wrote MODULE.bazel\n"), "{out}");
        assert!(!root.join("MODULE.bazel").exists());
        assert!(!root.join(".bazelversion").exists());
    }

    #[test]
    fn init_writes_the_module_into_a_repo_with_only_readme_license_and_gitignore() {
        let scratch = dx_test_scratch::scratch("dx-adopt-init-repo-files-");
        let root = scratch.path().to_path_buf();
        std::fs::write(root.join("README.md"), "mine\n").expect("readme");
        std::fs::write(root.join("LICENSE"), "license\n").expect("license");
        std::fs::write(root.join(".gitignore"), "target/\n").expect("ignore");
        let (code, out, err) = run(&invocation(&["init", "--apply"]), &root);
        assert_eq!(code, 0, "{err}");
        assert!(out.contains("wrote MODULE.bazel\n"), "{out}");
        assert!(root.join("MODULE.bazel").is_file());
        assert!(root.join(".bazelversion").is_file());
        assert!(!err.contains("treating this as an existing repo"), "{err}");
    }

    #[test]
    fn init_apply_writes_every_file_and_check_follows_them() {
        let scratch = dx_test_scratch::scratch("dx-adopt-init-full-");
        let root = scratch.path().to_path_buf();
        let (code, _out, err) = run(
            &invocation(&["init", "--apply", "--agents=claude,codex,gemini,copilot"]),
            &root,
        );
        assert_eq!(code, 0, "{err}");
        for path in [
            ".dx/pins",
            ".gitignore",
            ".bazelignore",
            "tools/dx",
            ".envrc",
            ".devcontainer/devcontainer.json",
            ".vscode/settings.json",
            ".github/workflows/ci.yml",
            ".claude/settings.json",
            "AGENTS.md",
            "CLAUDE.md",
        ] {
            assert!(root.join(path).exists(), "{path}");
        }
        let check = invocation(&["init"]);
        assert_eq!(run(&check, &root).0, 0);
        for path in [
            ".envrc",
            ".devcontainer/devcontainer.json",
            ".vscode/settings.json",
            ".github/workflows/ci.yml",
            "AGENTS.md",
            "CLAUDE.md",
        ] {
            std::fs::remove_file(root.join(path)).expect("remove");
            let (code, out, err) = run(&check, &root);
            assert_eq!(code, 0, "{path}: {out}{err}");
        }
        for path in [
            ".dx/pins",
            ".gitignore",
            ".bazelignore",
            "tools/dx",
            "tools/dx.cmd",
        ] {
            let saved = std::fs::read(root.join(path)).expect("read");
            std::fs::remove_file(root.join(path)).expect("remove");
            let (code, _out, err) = run(&check, &root);
            assert_eq!(code, 1, "{path}");
            assert!(
                err.contains(&format!("missing {path}")) || err.contains(path),
                "{path}: {err}"
            );
            std::fs::write(root.join(path), saved).expect("restore");
        }
    }

    fn tree(root: &std::path::Path) -> Vec<(String, Vec<u8>)> {
        let mut files = Vec::new();
        let mut stack = vec![root.to_path_buf()];
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(dir).expect("dir").flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else {
                    let name = path.strip_prefix(root).expect("under root");
                    files.push((
                        name.to_string_lossy().into_owned(),
                        std::fs::read(&path).expect("read"),
                    ));
                }
            }
        }
        files.sort();
        files
    }

    #[test]
    fn a_second_apply_leaves_the_tree_byte_identical() {
        let scratch = dx_test_scratch::scratch("dx-adopt-init-twice-");
        let root = scratch.path().to_path_buf();
        let inv = invocation(&["init", "--apply"]);
        assert_eq!(run(&inv, &root).0, 0);
        let before = tree(&root);
        let (code, out, err) = run(&inv, &root);
        assert_eq!(code, 0, "{err}");
        assert!(!out.contains("wrote "), "{out}");
        assert_eq!(tree(&root), before);
    }

    #[test]
    fn apply_rewrites_a_stale_ci_pin_and_editor_key_in_place() {
        let scratch = dx_test_scratch::scratch("dx-adopt-init-stale-pin-");
        let root = scratch.path().to_path_buf();
        let inv = invocation(&["init", "--apply"]);
        assert_eq!(run(&inv, &root).0, 0);
        let ci = root.join(".github/workflows/ci.yml");
        let fresh = std::fs::read_to_string(&ci).expect("read");
        let stale = fresh.replace(dx_adopt::CONSUMER_WORKFLOW_PIN, "0.0.1");
        std::fs::write(&ci, &stale).expect("stale");
        let (code, out, err) = run(&inv, &root);
        assert_eq!(code, 0, "{err}");
        assert!(out.contains("wrote .github/workflows/ci.yml"), "{out}");
        assert_eq!(std::fs::read_to_string(&ci).expect("read"), fresh);
    }
}
