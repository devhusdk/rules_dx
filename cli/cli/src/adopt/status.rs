use std::io::Write;

use clap::ValueEnum;

use crate::exec::common::{check_stdout_write, emit_built, emit_event};
use dx_output::{
    CapabilityEvent, ConfigFileEvent, FamilyToolsEvent, FinishedCounts, OutputMode, OwnedFileEvent,
    StatusEvent, TierEvent, capability_event, command_finished, command_started, config_file_event,
    family_tools_event, owned_file_event, status_event, tier_event,
};
use dx_plan::args::grammar::advertises_passthrough;
use dx_plan::args::{Command, Invocation};

pub(crate) const CODE_STATUS_CHECK_FAILED: &str = "status_check_failed";

use super::operational;

fn effect_of(command: Command) -> &'static str {
    if command.supports_apply() {
        "apply"
    } else {
        "none"
    }
}

fn output_names(command: Command) -> Vec<String> {
    let mut modes = vec!["text".to_owned()];
    if command.supports_json() {
        modes.push("json".to_owned());
    }
    modes
}

fn command_capability(command: Command) -> CapabilityEvent {
    let spec = dx_plan::plan::spec(command);
    CapabilityEvent {
        name: command.name().to_owned(),
        source: "cli-grammar".to_owned(),
        availability: "available".to_owned(),
        detail: command.describe().to_owned(),
        flags: dx_plan::args::grammar::advertised_flags(command)
            .into_iter()
            .map(|flag| format!("--{flag}"))
            .collect(),
        outputs: output_names(command),
        reports: dx_plan::reports::format_names(spec.reports)
            .into_iter()
            .map(ToOwned::to_owned)
            .collect(),
        scope_policy: command.scope_policy().to_owned(),
        effect: effect_of(command).to_owned(),
        passthrough: advertises_passthrough(command),
    }
}

fn tools_capability(tools: &dx_adopt::StatusCheck) -> CapabilityEvent {
    let availability = match tools.status.as_str() {
        "ok" => "available",
        "error" => "unavailable",
        _ => "unknown",
    };
    CapabilityEvent {
        name: "workspace:tools".to_owned(),
        source: "workspace-record".to_owned(),
        availability: availability.to_owned(),
        detail: tools.detail.clone(),
        flags: Vec::new(),
        outputs: Vec::new(),
        reports: Vec::new(),
        scope_policy: "reject".to_owned(),
        effect: "none".to_owned(),
        passthrough: false,
    }
}

fn capabilities(tools: &dx_adopt::StatusCheck) -> Vec<CapabilityEvent> {
    let mut records: Vec<CapabilityEvent> = Command::value_variants()
        .iter()
        .map(|command| command_capability(*command))
        .collect();
    records.push(tools_capability(tools));
    records
}

fn failure(checks: &[dx_adopt::StatusCheck]) -> Option<(&'static str, String)> {
    let failed: Vec<&str> = checks
        .iter()
        .filter(|check| check.status == "error")
        .map(|check| check.name.as_str())
        .collect();
    if failed.is_empty() {
        return None;
    }
    Some((
        CODE_STATUS_CHECK_FAILED,
        format!("failed checks: {} (see hint)", failed.join(", ")),
    ))
}

fn capability_line(capability: &CapabilityEvent) -> String {
    let join = |items: &[String], separator: &str| {
        if items.is_empty() {
            "none".to_owned()
        } else {
            items.join(separator)
        }
    };
    format!(
        "capability {}: {} [source: {}; availability: {}; flags: {}; output: {}; reports: {}; scopes: {}; effect: {}; passthrough: {}]",
        capability.name,
        capability.detail,
        capability.source,
        capability.availability,
        join(&capability.flags, " "),
        join(&capability.outputs, "|"),
        join(&capability.reports, "|"),
        capability.scope_policy,
        capability.effect,
        if capability.passthrough { "yes" } else { "no" },
    )
}

fn init_check(workspace: &std::path::Path) -> dx_adopt::StatusCheck {
    let drift =
        match super::init_files::plan(workspace, "refresh", dx_adopt::is_fresh_dir(workspace), &[])
        {
            Ok(files) => super::init_files::drift(workspace, &files),
            Err(error) => super::init_files::Drift {
                other: vec![error.to_string()],
                ..Default::default()
            },
        };
    let mut parts: Vec<String> = Vec::new();
    if !drift.missing.is_empty() {
        parts.push(format!("missing {}", drift.missing.join(", ")));
    }
    if !drift.stale.is_empty() {
        parts.push(format!("stale {}", drift.stale.join(", ")));
    }
    parts.extend(drift.other);
    let (status, detail) = if parts.is_empty() {
        ("ok", "every file dx init writes is current".to_owned())
    } else {
        let broken = drift
            .stale
            .iter()
            .any(|path| path == dx_adopt::LAUNCHER_SH_REL || path == dx_adopt::LAUNCHER_CMD_REL);
        (if broken { "error" } else { "warn" }, parts.join("; "))
    };
    dx_adopt::StatusCheck {
        name: "init".to_owned(),
        status: status.to_owned(),
        detail,
        hint: "dx init --apply writes the missing and stale files".to_owned(),
    }
}

/// One file dx init writes, its ownership label and whether it is in step with this dx.
fn owned_files(workspace: &std::path::Path) -> Result<Vec<OwnedFileEvent>, dx_error::DxError> {
    let files =
        super::init_files::plan(workspace, "refresh", dx_adopt::is_fresh_dir(workspace), &[])
            .map_err(|error| super::from_adopt("", &error))?;
    let drift = super::init_files::drift(workspace, &files);
    let named = |list: &[String], path: &str| {
        list.iter().any(|entry| {
            entry == path
                || entry
                    .strip_prefix(path)
                    .is_some_and(|rest| rest.starts_with(' '))
        })
    };
    Ok(files
        .iter()
        .map(|file| {
            let owns = file.ownership();
            let state = if named(&drift.missing, &file.path) {
                "missing"
            } else if named(&drift.absent, &file.path) {
                "absent"
            } else if named(&drift.stale, &file.path) {
                "stale"
            } else if owns == dx_adopt::Ownership::Once {
                "yours"
            } else {
                "current"
            };
            OwnedFileEvent {
                path: file.path.clone(),
                owns: owns.label().to_owned(),
                state: state.to_owned(),
            }
        })
        .collect())
}

/// The tiers in force, the root tool configs bound to `.dx`, and any `dx.toml` problem.
struct Bindings {
    tiers: Vec<TierEvent>,
    configs: Vec<ConfigFileEvent>,
    tools: Vec<FamilyToolsEvent>,
    problem: Option<dx_adopt::StatusCheck>,
}

/// The event vocabulary for a check status.
fn severity(status: &str) -> &str {
    match status {
        "ok" => "info",
        "warn" => "warning",
        other => other,
    }
}

fn problem_check(name: &str, detail: String) -> dx_adopt::StatusCheck {
    dx_adopt::StatusCheck {
        name: name.to_owned(),
        status: "warn".to_owned(),
        detail,
        hint: format!("fix the {name} problem, then run dx status"),
    }
}

/// The tools each detected, enabled family runs per capability, after `dx.toml` edits.
fn family_tools(
    selection: &dx_config::Selection,
    detected: &std::collections::BTreeSet<String>,
) -> Vec<FamilyToolsEvent> {
    let mut events = Vec::new();
    for (id, family) in &selection.families {
        if !detected.contains(id) {
            continue;
        }
        let capabilities = [
            ("lint", &family.lint),
            ("format", &family.format),
            ("typecheck", &family.typecheck),
            ("audit", &family.audit),
        ];
        for (capability, tools) in capabilities {
            if !tools.is_empty() {
                events.push(FamilyToolsEvent {
                    family: id.clone(),
                    capability: capability.to_owned(),
                    tools: tools.clone(),
                });
            }
        }
    }
    events
}

fn bindings(workspace: &std::path::Path) -> Bindings {
    let config = match dx_config::load(workspace) {
        Ok(config) => config,
        Err(error) => {
            return Bindings {
                tiers: Vec::new(),
                configs: Vec::new(),
                tools: Vec::new(),
                problem: Some(problem_check("dx.toml", error.to_string())),
            };
        }
    };
    let families = dx_adopt::detected_families(workspace);
    let explicit = config.as_ref().and_then(|config| config.profile);
    let from = |profile: Option<dx_config::Profile>, family: Option<String>| TierEvent {
        family,
        tier: profile.unwrap_or_default().as_str().to_owned(),
        source: if profile.is_some() {
            "dx.toml"
        } else {
            "default"
        }
        .to_owned(),
    };
    let mut tiers = vec![from(explicit, None)];
    if let Some(config) = &config {
        tiers.extend(config.families.iter().filter_map(|(id, family)| {
            let profile = family.profile?;
            (!family.disabled && families.contains(id))
                .then(|| from(Some(profile), Some(id.clone())))
        }));
    }
    let mut problem = None;
    let tools = match dx_config::effective(config.as_ref()) {
        Ok(selection) => family_tools(&selection, &families),
        Err(error) => {
            problem = Some(problem_check("dx.toml", error.to_string()));
            Vec::new()
        }
    };
    let mut root_files = std::collections::BTreeSet::new();
    match std::fs::read_dir(workspace) {
        Ok(entries) => {
            root_files.extend(
                entries.filter_map(|entry| {
                    Some(entry.ok()?.file_name().to_string_lossy().into_owned())
                }),
            );
        }
        Err(error) => {
            problem = Some(problem_check(
                "workspace",
                format!("cannot list the workspace: {error}"),
            ))
        }
    }
    let configs = match dx_generate::bound_configs(config.as_ref(), &families, &root_files) {
        Ok(configs) => configs
            .into_iter()
            .map(|(tool, file)| ConfigFileEvent {
                tool: tool.to_owned(),
                file,
                role: "root".to_owned(),
                base: None,
                edited: None,
            })
            .collect(),
        Err(error) => {
            problem = Some(problem_check("dx.toml", error.to_string()));
            Vec::new()
        }
    };
    Bindings {
        tiers,
        configs,
        tools,
        problem,
    }
}

fn lease_check(workspace: &std::path::Path) -> Option<dx_adopt::StatusCheck> {
    let (path, record) = dx_worktree::lease_for_workspace(workspace)?;
    let detail = match (record.holder(true), &record.branch) {
        (Some(holder), Some(branch)) => format!("{} held by {holder} on {branch}", path.display()),
        (Some(holder), None) => format!("{} held by {holder}", path.display()),
        (None, _) => format!("{} is free", path.display()),
    };
    Some(dx_adopt::StatusCheck {
        name: "lease".to_owned(),
        status: "ok".to_owned(),
        detail,
        hint: "dx worktree release frees this slot".to_owned(),
    })
}

fn slots_check(dir: &std::path::Path) -> Option<dx_adopt::StatusCheck> {
    let bad = dx_plan::slots::unusable_slot_files(dir);
    if bad.is_empty() {
        return None;
    }
    let list: Vec<String> = bad
        .iter()
        .map(|(path, error)| format!("{} ({error})", path.display()))
        .collect();
    Some(dx_adopt::StatusCheck {
        name: "slots".to_owned(),
        status: "warn".to_owned(),
        detail: format!("dx cannot use slot files: {}", list.join(", ")),
        hint: "fix the permissions of or remove these slot files".to_owned(),
    })
}

fn share_check(
    workspace: &std::path::Path,
    env_get: &dyn Fn(&str) -> Option<String>,
) -> Option<dx_adopt::StatusCheck> {
    let report =
        dx_plan::slots::share_report(workspace, env_get, dx_adopt::defaults::BuildSlots::Auto)?;
    if report.held == 0 {
        return None;
    }
    Some(dx_adopt::StatusCheck {
        name: "share".to_owned(),
        status: "ok".to_owned(),
        detail: format!(
            "{} of {} build slots held; a new run gets {} cpus and {} MB",
            report.held, report.count, report.next.cpus, report.next.memory_mb
        ),
        hint: "dx splits the machine across held build slots".to_owned(),
    })
}

pub(crate) fn execute_status(
    invocation: &Invocation,
    workspace: &std::path::Path,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let tools = super::status_tools::managed_tools_check(workspace);
    let records = if invocation.capabilities {
        capabilities(&tools)
    } else {
        Vec::new()
    };
    let owned = if invocation.files {
        match owned_files(workspace) {
            Ok(files) => files,
            Err(error) => {
                return crate::exec::common::report_dx_error(
                    invocation.output,
                    out,
                    err,
                    "init_failed",
                    &error,
                );
            }
        }
    } else {
        Vec::new()
    };
    let mut bound = bindings(workspace);
    let mut checks =
        dx_adopt::observed_status_checks(workspace, dx_adopt::read_version_pin(workspace), tools);
    checks.extend(bound.problem.take());
    checks.push(init_check(workspace));
    if let Some(check) = lease_check(workspace) {
        checks.push(check);
    }
    let env_get = |name: &str| std::env::var(name).ok();
    if let Some(check) = dx_plan::slots::slots_dir(&env_get).and_then(|dir| slots_check(&dir)) {
        checks.push(check);
    }
    if let Some(check) = share_check(workspace, &env_get) {
        checks.push(check);
    }
    if invocation.output == OutputMode::Json {
        if let Err(exit) = emit_built(out, command_started(invocation.command.name(), "default")) {
            return exit;
        }
        for check in &checks {
            if let Err(exit) = emit_built(
                out,
                status_event(&StatusEvent {
                    name: check.name.clone(),
                    severity: severity(&check.status).to_owned(),
                    detail: check.detail.clone(),
                    hint: check.hint.clone(),
                }),
            ) {
                return exit;
            }
        }
        let events = bound
            .tiers
            .iter()
            .map(tier_event)
            .chain(bound.configs.iter().map(config_file_event))
            .chain(bound.tools.iter().map(family_tools_event));
        for event in events {
            match event {
                Ok(event) => {
                    if let Err(exit) = emit_event(out, &event) {
                        return exit;
                    }
                }
                Err(error) => {
                    return operational(invocation, out, err, "invalid_result", &error.to_string());
                }
            }
        }
        for capability in &records {
            match capability_event(capability) {
                Ok(event) => {
                    if let Err(exit) = emit_event(out, &event) {
                        return exit;
                    }
                }
                Err(error) => {
                    return operational(invocation, out, err, "invalid_result", &error.to_string());
                }
            }
        }
        for file in &owned {
            match owned_file_event(file) {
                Ok(event) => {
                    if let Err(exit) = emit_event(out, &event) {
                        return exit;
                    }
                }
                Err(error) => {
                    return operational(invocation, out, err, "invalid_result", &error.to_string());
                }
            }
        }
        if let Some((code, message)) = failure(&checks) {
            return operational(invocation, out, err, code, &message);
        }
        if let Err(exit) = emit_event(out, &command_finished(0, &FinishedCounts::default())) {
            return exit;
        }
        return 0;
    }
    if let Err(exit) =
        check_stdout_write(writeln!(out, "{}", dx_adopt::render_status_text(&checks)))
    {
        return exit;
    }
    let tier_lines = bound.tiers.iter().map(|tier| match &tier.family {
        Some(family) => format!("tier {family}: {} [source: {}]", tier.tier, tier.source),
        None => format!("tier: {} [source: {}]", tier.tier, tier.source),
    });
    let config_lines = bound
        .configs
        .iter()
        .map(|config| format!("config {}: {}", config.tool, config.file));
    let tool_lines = bound.tools.iter().map(|entry| {
        format!(
            "tools {} {}: {}",
            entry.family,
            entry.capability,
            entry.tools.join(", ")
        )
    });
    for line in tier_lines.chain(config_lines).chain(tool_lines) {
        if let Err(exit) = check_stdout_write(writeln!(out, "{line}")) {
            return exit;
        }
    }
    for capability in &records {
        if let Err(exit) = check_stdout_write(writeln!(out, "{}", capability_line(capability))) {
            return exit;
        }
    }
    for file in &owned {
        if let Err(exit) = check_stdout_write(writeln!(
            out,
            "file {} {} {}",
            file.path, file.owns, file.state
        )) {
            return exit;
        }
    }
    if let Some((code, message)) = failure(&checks) {
        return operational(invocation, out, err, code, &message);
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adopt::test_support::{event, event_kinds, invocation, json_events, run};
    use std::io;

    #[test]
    fn owned_list_covers_launcher_preset_and_override_paths() {
        let scratch = dx_test_scratch::scratch("dx-status-owned-");
        let owned: Vec<String> = crate::adopt::init_files::plan(
            scratch.path(),
            "my_project",
            true,
            &dx_plan::args::Agent::ALL,
        )
        .expect("plan")
        .into_iter()
        .map(|file| file.path)
        .collect();
        for path in [dx_adopt::LAUNCHER_SH_REL, dx_adopt::LAUNCHER_CMD_REL] {
            assert!(owned.iter().any(|p| p == path), "{path}");
        }
        for path in dx_adopt::preset_paths() {
            assert!(owned.contains(&path), "{path}");
        }
    }

    fn write_module(root: &std::path::Path) {
        std::fs::write(
            root.join("MODULE.bazel"),
            "bazel_dep(name = \"rules_dx\", version = \"0.0.0\")\n",
        )
        .expect("module");
    }

    #[test]
    fn status_inside_a_leased_slot_shows_the_lease_and_its_holder() {
        let scratch = dx_test_scratch::scratch("dx-adopt-status-lease-");
        let pool = scratch.path().join("pool");
        let slot = pool.join("3");
        std::fs::create_dir_all(&slot).expect("slot");
        std::fs::create_dir_all(pool.join(".dx")).expect("meta");
        let record = dx_worktree::pool::Record {
            slot: 3,
            held: true,
            branch: Some("feature/a".to_owned()),
            last_branch: None,
            session: Some("s1".to_owned()),
            agent: None,
            pid: Some(7),
            pid_start: None,
            boot_id: None,
            gone: false,
            stopped_at: None,
            unclaimed: false,
            broken: false,
            leased_at: 0,
            last_used: 0,
            lock_digest: None,
        };
        std::fs::write(
            pool.join(".dx").join("3.json"),
            serde_json::to_vec(&record).expect("json"),
        )
        .expect("record");
        let check = lease_check(&slot).expect("lease check");
        assert_eq!(check.name, "lease");
        assert!(
            check.detail.contains("session s1 pid 7"),
            "{}",
            check.detail
        );
        assert!(check.detail.contains("feature/a"), "{}", check.detail);
        assert!(lease_check(scratch.path()).is_none());
    }

    #[test]
    fn slots_check_names_unusable_slot_files_and_creates_nothing() {
        let scratch = dx_test_scratch::scratch("dx-adopt-status-slots-");
        let dir = scratch.path();
        assert!(slots_check(dir).is_none());
        std::fs::write(dir.join("slot-0.lock"), "").expect("file");
        std::fs::create_dir(dir.join("slot-1.lock")).expect("dir");
        std::fs::write(dir.join("other.txt"), "").expect("other");
        let check = slots_check(dir).expect("slots check");
        assert_eq!(check.name, "slots");
        assert_eq!(check.status, "warn");
        assert!(check.detail.contains("slot-1.lock"), "{}", check.detail);
        assert!(!check.detail.contains("slot-0.lock"), "{}", check.detail);
        assert_eq!(std::fs::read_dir(dir).expect("dir").count(), 3);
    }

    #[test]
    fn status_flags_a_missing_or_stale_agents_block_and_accepts_a_current_one() {
        let scratch = dx_test_scratch::scratch("dx-adopt-status-agents-");
        let root = scratch.path();
        let check = init_check(root);
        assert_eq!(
            (check.name.as_str(), check.status.as_str()),
            ("init", "warn")
        );
        assert!(check.detail.contains("tools/dx"), "{}", check.detail);
        let (code, _out, err) = crate::adopt::test_support::run(
            &crate::adopt::test_support::invocation(&[
                "init",
                "--apply",
                "--agents=claude,codex,gemini,copilot",
            ]),
            root,
        );
        assert_eq!(code, 0, "{err}");
        assert_eq!(init_check(root).status, "ok");
        std::fs::write(
            root.join("AGENTS.md"),
            "# Mine\n<!-- dx:begin -->\n- old\n<!-- dx:end -->\n",
        )
        .expect("stale");
        let check = init_check(root);
        assert_eq!(check.status, "warn");
        assert!(check.detail.contains("stale AGENTS.md"), "{}", check.detail);
        std::fs::write(root.join("AGENTS.md"), super::super::agents::block()).expect("current");
        std::fs::remove_file(root.join(".claude/settings.json")).expect("rm");
        assert_eq!(init_check(root).status, "ok");
        std::fs::write(
            root.join(".claude/settings.json"),
            dx_worktree::setup::settings_text(&dx_worktree::setup::CLAUDE),
        )
        .expect("settings");
        std::fs::write(root.join("CLAUDE.md"), "# Mine\n").expect("claude");
        let check = init_check(root);
        assert!(check.detail.contains("CLAUDE.md"), "{}", check.detail);
        std::fs::write(root.join("CLAUDE.md"), "@AGENTS.md\n").expect("claude");
        assert_eq!(init_check(root).status, "ok");
        #[cfg(unix)]
        {
            std::fs::remove_file(root.join("CLAUDE.md")).expect("rm");
            std::os::unix::fs::symlink("AGENTS.md", root.join("CLAUDE.md")).expect("link");
            assert_eq!(init_check(root).status, "ok");
        }
        std::fs::create_dir_all(root.join(".codex")).expect("dir");
        std::fs::write(root.join(".codex/hooks.json"), "{}\n").expect("codex");
        let check = init_check(root);
        assert_eq!(check.status, "warn");
        assert!(
            check
                .detail
                .contains(".codex/hooks.json (lacks hooks: PreToolUse (Bash)"),
            "{}",
            check.detail
        );
        assert!(!check.detail.contains(".claude"), "{}", check.detail);
        std::fs::write(
            root.join(".codex/hooks.json"),
            dx_worktree::setup::settings_text(&dx_worktree::setup::CODEX),
        )
        .expect("codex");
        assert_eq!(init_check(root).status, "ok");
    }

    #[test]
    fn status_reports_pin_and_checks() {
        let inv = invocation(&["status"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-status-status-");
        let root = scratch.path().to_path_buf();
        std::fs::create_dir_all(root.join(".dx")).expect("dx");
        std::fs::write(root.join(".dx/pins"), "0.0.0\n").expect("pin");
        write_module(&root);
        let (code, out, _err) = run(&inv, &root);
        assert_eq!(code, 0);
        assert!(out.contains("pin: ok"));
    }

    #[test]
    fn status_fails_on_a_stale_launcher() {
        let inv = invocation(&["status"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-status-launcher-");
        let root = scratch.path().to_path_buf();
        std::fs::create_dir_all(root.join(".dx")).expect("dx");
        std::fs::write(root.join(".dx/pins"), "0.0.0\n").expect("pin");
        write_module(&root);
        dx_adopt::write_launcher_files(&root, &mut dx_adopt::Applied::default())
            .expect("launchers");
        let (code, out, _err) = run(&inv, &root);
        assert_eq!(code, 0, "{out}");
        assert!(!out.contains("init: error"), "{out}");
        std::fs::write(
            root.join("tools/dx"),
            "#!/bin/sh\nset -eu\nDX_PINS=.dx/pins\n",
        )
        .expect("stale");
        let (code, out, err) = run(&inv, &root);
        assert_eq!(code, 1, "{out}");
        assert!(
            out.contains("init: error (") && out.contains("stale tools/dx"),
            "{out}"
        );
        assert!(err.contains("status_check_failed"), "{err}");
        assert!(err.contains("failed checks: init"), "{err}");
    }

    #[test]
    fn status_json_streams_envelope() {
        let inv = invocation(&["status", "--output=json"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-status-json-");
        let root = scratch.path().to_path_buf();
        std::fs::create_dir_all(root.join(".dx")).expect("dx");
        std::fs::write(root.join(".dx/pins"), "0.0.0\n").expect("pin");
        write_module(&root);
        let (code, out, err) = run(&inv, &root);
        assert_eq!(code, 0);
        let text = out;
        let events = json_events(&text);
        assert!(!events.is_empty());
        for event in &events {
            assert!(event.get("schema").is_some(), "{event}");
            assert!(event.get("event").is_some(), "{event}");
        }
        let kinds = event_kinds(&events);
        assert_eq!(kinds[0], "command_started");
        assert_eq!(kinds[kinds.len() - 1], "command_finished");
        assert!(kinds.contains(&"status"), "{kinds:?}");
        assert!(!kinds.contains(&"error"), "{kinds:?}");
        assert_eq!(
            events.last().expect("finished")["exit_code"],
            serde_json::json!(0)
        );
        let status = events
            .iter()
            .find(|event| {
                event["event"] == serde_json::json!("status")
                    && event["name"] == serde_json::json!("pin")
            })
            .expect("pin status event");
        assert_eq!(status["severity"], serde_json::json!("info"));
        assert!(err.is_empty());
    }

    fn status_with(files: &[(&str, &str)], words: &[&str]) -> String {
        let scratch = dx_test_scratch::scratch("dx-adopt-status-tier-");
        let root = scratch.path().to_path_buf();
        write_module(&root);
        for (name, text) in files {
            std::fs::write(root.join(name), text).expect("file");
        }
        run(&invocation(words), &root).1
    }

    #[test]
    fn status_without_dx_toml_is_the_default_standard_tier() {
        let out = status_with(&[], &["status"]);
        assert!(out.contains("tier: standard [source: default]\n"), "{out}");
        assert!(!out.contains("\nconfig "), "{out}");
    }

    #[test]
    fn status_reads_the_tier_from_dx_toml() {
        let out = status_with(
            &[("dx.toml", "schema_version = 1\ntier = \"strict\"\n")],
            &["status"],
        );
        assert!(out.contains("tier: strict [source: dx.toml]\n"), "{out}");
    }

    #[test]
    fn status_shows_a_family_tier_override() {
        let files = [
            ("a.py", "x = 1\n"),
            (
                "dx.toml",
                "schema_version = 1\n[families.python]\ntier = \"strict\"\n",
            ),
        ];
        let out = status_with(&files, &["status"]);
        assert!(out.contains("tier: standard [source: default]\n"), "{out}");
        assert!(
            out.contains("tier python: strict [source: dx.toml]\n"),
            "{out}"
        );
        let json = json_events(&status_with(&files, &["status", "--output=json"]));
        let family = json
            .iter()
            .find(|event| event["event"] == "tier" && event["family"] == "python")
            .expect("family tier event");
        assert_eq!(family["tier"], "strict");
    }

    #[test]
    fn status_lists_the_effective_tools_per_family() {
        let files = [
            ("a.py", "x = 1\n"),
            (
                "dx.toml",
                "schema_version = 1\nskip_tools = [\"typos\"]\n[families.python]\nformat = [\"-ruff\"]\n",
            ),
        ];
        let out = status_with(&files, &["status"]);
        assert!(out.contains("tools python lint: ruff\n"), "{out}");
        assert!(out.contains("tools python typecheck: ty\n"), "{out}");
        assert!(!out.contains("tools python format"), "{out}");
        let json = json_events(&status_with(&files, &["status", "--output=json"]));
        let lint = json
            .iter()
            .find(|event| {
                event["event"] == "tools"
                    && event["family"] == "python"
                    && event["capability"] == "lint"
            })
            .expect("python lint tools event");
        assert_eq!(lint["tools"], serde_json::json!(["ruff"]));
    }

    #[test]
    fn status_skips_the_tier_of_a_disabled_family() {
        let files = [
            ("a.py", "x = 1\n"),
            (
                "dx.toml",
                "schema_version = 1\n[families.python]\ntier = \"strict\"\ndisabled = true\n",
            ),
        ];
        let out = status_with(&files, &["status"]);
        assert!(!out.contains("tier python"), "{out}");
    }

    #[test]
    fn status_warns_about_an_unparsable_dx_toml() {
        let files = [("dx.toml", "tier = [\n")];
        let out = status_with(&files, &["status"]);
        assert!(out.contains("dx.toml: warn"), "{out}");
        let json = json_events(&status_with(&files, &["status", "--output=json"]));
        let check = json
            .iter()
            .find(|event| event["event"] == "status" && event["name"] == "dx.toml")
            .expect("dx.toml status event");
        assert_eq!(check["severity"], "warning");
    }

    #[test]
    fn status_warns_about_an_unknown_family() {
        let files = [(
            "dx.toml",
            "schema_version = 1\n[families.nosuchfamily]\ntier = \"strict\"\n",
        )];
        let out = status_with(&files, &["status"]);
        assert!(out.contains("dx.toml: warn"), "{out}");
        assert!(out.contains("unknown family `nosuchfamily`"), "{out}");
        assert!(out.contains("tier: standard [source: default]\n"), "{out}");
    }

    #[test]
    fn status_lists_the_root_config_of_a_detected_tool() {
        let files = [("a.py", "x = 1\n"), ("ruff.toml", "")];
        let out = status_with(&files, &["status"]);
        assert!(out.contains("config ruff: ruff.toml\n"), "{out}");
        let json = json_events(&status_with(&files, &["status", "--output=json"]));
        let tier = event(&json, "tier");
        assert_eq!(tier["tier"], "standard");
        assert_eq!(tier["source"], "default");
        let config = event(&json, "config_file");
        assert_eq!(config["tool"], "ruff");
        assert_eq!(config["file"], "ruff.toml");
    }

    fn assert_pin_warns(words: &[&str], root: &std::path::Path) {
        let inv = invocation(words);
        let (code, out, err) = run(&inv, root);
        assert_eq!(code, 0, "words: {words:?}: {err}");
        assert!(err.is_empty(), "{err}");
        if words.contains(&"--output=json") {
            let events = json_events(&out);
            let pin = events
                .iter()
                .find(|event| event["name"] == serde_json::json!("pin"))
                .expect("pin status event");
            assert_eq!(pin["severity"], serde_json::json!("warning"));
            assert_eq!(pin["hint"], serde_json::json!("dx init --apply"));
        } else {
            assert!(out.contains("pin: warn"), "{out}");
        }
    }

    #[test]
    fn status_other_version_pin_warns() {
        for words in [vec!["status"], vec!["status", "--output=json"]] {
            let scratch = dx_test_scratch::scratch("dx-adopt-status-mismatch-");
            let root = scratch.path().to_path_buf();
            std::fs::create_dir_all(root.join(".dx")).expect("dx");
            std::fs::write(root.join(".dx/pins"), "9.9.9\n").expect("pin");
            write_module(&root);
            assert_pin_warns(&words, &root);
        }
    }

    #[test]
    fn status_missing_pin_warns() {
        for words in [vec!["status"], vec!["status", "--output=json"]] {
            let scratch = dx_test_scratch::scratch("dx-adopt-status-missing-");
            let root = scratch.path().to_path_buf();
            write_module(&root);
            assert_pin_warns(&words, &root);
        }
    }

    #[test]
    fn status_empty_pin_warns() {
        let scratch = dx_test_scratch::scratch("dx-adopt-status-empty-");
        let root = scratch.path().to_path_buf();
        std::fs::create_dir_all(root.join(".dx")).expect("dx");
        std::fs::write(root.join(".dx/pins"), "\n").expect("empty pin");
        write_module(&root);
        assert_pin_warns(&["status"], &root);
    }

    #[test]
    fn status_text_lists_every_command_and_workspace_tools() {
        let inv = invocation(&["status", "--capabilities"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-status-cap-text-");
        let root = scratch.path().to_path_buf();
        std::fs::create_dir_all(root.join(".dx")).expect("dx");
        std::fs::write(root.join(".dx/pins"), "0.0.0\n").expect("pin");
        write_module(&root);
        let (code, out, err) = run(&inv, &root);
        assert_eq!(code, 0, "{err}");
        for command in Command::value_variants() {
            assert!(
                out.contains(&format!("capability {}:", command.name())),
                "{out}"
            );
        }
        assert!(!out.contains("capability capabilities:"), "{out}");
        assert!(
            out.contains("capability workspace:tools:") && out.contains("availability: unknown"),
            "{out}"
        );
    }

    fn state_of(out: &str, path: &str) -> String {
        out.lines()
            .find_map(|line| {
                let words: Vec<&str> = line.split(' ').collect();
                (words.len() == 4 && words[0] == "file" && words[1] == path)
                    .then(|| format!("{} {}", words[2], words[3]))
            })
            .unwrap_or_else(|| panic!("no file line for {path}: {out}"))
    }

    #[test]
    fn status_files_lists_every_planned_file_and_flags_a_stale_dx_key() {
        let scratch = dx_test_scratch::scratch("dx-adopt-status-files-");
        let root = scratch.path().to_path_buf();
        let (code, _out, err) = run(&invocation(&["init", "--apply"]), &root);
        assert_eq!(code, 0, "{err}");
        let (code, out, err) = run(&invocation(&["status", "--files"]), &root);
        assert_eq!(code, 0, "{err}");
        let planned = crate::adopt::init_files::plan(&root, "refresh", false, &[]).expect("plan");
        for file in &planned {
            let state = state_of(&out, &file.path);
            let expected = if file.ownership() == dx_adopt::Ownership::Once {
                "yours"
            } else {
                "current"
            };
            assert_eq!(
                state,
                format!("{} {expected}", file.ownership().label()),
                "{}",
                file.path
            );
        }
        let settings = root.join(".vscode/settings.json");
        let mut value: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&settings).expect("settings"))
                .expect("json");
        value["files.watcherExclude"]["**/bazel-*/**"] = serde_json::json!(false);
        std::fs::write(&settings, value.to_string()).expect("edit");
        let (_code, out, _err) = run(&invocation(&["status", "--files"]), &root);
        assert_eq!(state_of(&out, ".vscode/settings.json"), "keys stale");
        let (_code, out, _err) = run(&invocation(&["status", "--files", "--output=json"]), &root);
        let events = json_events(&out);
        let owned = crate::adopt::test_support::events_of_kind(&events, "owned_file");
        assert_eq!(owned.len(), planned.len());
        assert!(owned.iter().any(|event| {
            event["path"] == ".vscode/settings.json"
                && event["owns"] == "keys"
                && event["state"] == "stale"
        }));
    }

    #[test]
    fn status_default_output_omits_file_records() {
        let scratch = dx_test_scratch::scratch("dx-adopt-status-nofiles-");
        let root = scratch.path().to_path_buf();
        let (code, _out, err) = run(&invocation(&["init", "--apply"]), &root);
        assert_eq!(code, 0, "{err}");
        let (_code, out, _err) = run(&invocation(&["status"]), &root);
        assert!(!out.contains("\nfile "), "{out}");
    }

    #[test]
    fn status_default_output_omits_capability_records() {
        let scratch = dx_test_scratch::scratch("dx-adopt-status-compact-");
        let root = scratch.path().to_path_buf();
        std::fs::create_dir_all(root.join(".dx")).expect("dx");
        std::fs::write(root.join(".dx/pins"), "0.0.0\n").expect("pin");
        write_module(&root);
        let json = invocation(&["status", "--output=json"]);
        let (code, out, err) = run(&json, &root);
        assert_eq!(code, 0, "{err}");
        let events = json_events(&out);
        let kinds = event_kinds(&events);
        assert!(!kinds.contains(&"capability"), "{kinds:?}");
        assert!(kinds.contains(&"status"), "{kinds:?}");
        assert_eq!(kinds[0], "command_started");
        assert_eq!(kinds[kinds.len() - 1], "command_finished");
        let text = invocation(&["status"]);
        let (code, out, err) = run(&text, &root);
        assert_eq!(code, 0, "{err}");
        assert!(!out.contains("capability "), "{out}");
    }

    #[test]
    fn status_json_verbose_emits_no_capability_records() {
        let inv = invocation(&["status", "--output=json", "--verbose"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-status-verbose-");
        let root = scratch.path().to_path_buf();
        std::fs::create_dir_all(root.join(".dx")).expect("dx");
        std::fs::write(root.join(".dx/pins"), "0.0.0\n").expect("pin");
        write_module(&root);
        let (code, out, err) = run(&inv, &root);
        assert_eq!(code, 0, "{err}");
        let events = json_events(&out);
        let capabilities = crate::adopt::test_support::events_of_kind(&events, "capability");
        assert!(capabilities.is_empty());
    }

    #[test]
    fn status_json_streams_capability_records() {
        let inv = invocation(&["status", "--output=json", "--capabilities"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-status-cap-json-");
        let root = scratch.path().to_path_buf();
        std::fs::create_dir_all(root.join(".dx")).expect("dx");
        std::fs::write(root.join(".dx/pins"), "0.0.0\n").expect("pin");
        write_module(&root);
        let (code, out, err) = run(&inv, &root);
        assert_eq!(code, 0, "{err}");
        let events = json_events(&out);
        let capabilities = crate::adopt::test_support::events_of_kind(&events, "capability");
        assert_eq!(capabilities.len(), Command::value_variants().len() + 1);
        let lint = capabilities
            .iter()
            .find(|event| event["name"] == serde_json::json!("lint"))
            .expect("lint record");
        assert_eq!(lint["source"], serde_json::json!("cli-grammar"));
        for (name, effect) in [
            ("lint", "apply"),
            ("check", "apply"),
            ("build", "none"),
            ("status", "none"),
        ] {
            let record = capabilities
                .iter()
                .find(|event| event["name"] == serde_json::json!(name))
                .expect("capability record");
            assert_eq!(record["effect"], serde_json::json!(effect), "{name}");
        }
        assert_eq!(lint["availability"], serde_json::json!("available"));
        let tools = capabilities
            .iter()
            .find(|event| event["name"] == serde_json::json!("workspace:tools"))
            .expect("tools record");
        assert_eq!(tools["availability"], serde_json::json!("unknown"));
        assert_eq!(tools["source"], serde_json::json!("workspace-record"));
    }

    #[test]
    fn status_reports_actionable_health_checks() {
        let inv = invocation(&["status", "--output=json"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-status-health-");
        let root = scratch.path().to_path_buf();
        std::fs::create_dir_all(root.join(".dx")).expect("dx");
        std::fs::write(root.join(".dx/pins"), "0.0.0\n").expect("pin");
        std::fs::write(
            root.join("MODULE.bazel"),
            "bazel_dep(name = \"rules_dx\", version = \"9.9.9\")\n",
        )
        .expect("module");
        let before = std::fs::read_dir(&root).expect("dir").count();
        let (code, out, _) = run(&inv, &root);
        assert_eq!(code, 1);
        let events = json_events(&out);
        let module = events
            .iter()
            .find(|event| event["name"] == serde_json::json!("module"))
            .expect("module check");
        assert_eq!(module["severity"], serde_json::json!("error"));
        assert!(module["hint"].as_str().expect("hint").starts_with("dx "));
        for name in ["bazel", "host", "tools"] {
            assert!(
                events
                    .iter()
                    .any(|event| event["name"] == serde_json::json!(name)),
                "{name}"
            );
        }
        assert_eq!(std::fs::read_dir(&root).expect("dir").count(), before);
        let error = events
            .iter()
            .find(|event| event["event"] == serde_json::json!("error"))
            .expect("error event");
        assert_eq!(error["code"], serde_json::json!(CODE_STATUS_CHECK_FAILED));
        assert_eq!(
            error["message"],
            serde_json::json!("failed checks: module (see hint)")
        );
    }

    #[test]
    fn health_findings_report_check_failed() {
        for words in [vec!["status"], vec!["status", "--output=json"]] {
            let inv = invocation(&words);
            let scratch = dx_test_scratch::scratch("dx-adopt-status-bare-");
            let root = scratch.path().to_path_buf();
            std::fs::create_dir_all(root.join(".dx")).expect("dx");
            std::fs::write(root.join(".dx/pins"), "0.0.0\n").expect("pin");
            std::fs::write(root.join("MODULE.bazel"), "module(name = \"app\")\n").expect("module");
            let (code, out, err) = run(&inv, &root);
            assert_eq!(code, 1, "words: {words:?}");
            assert!(err.contains(CODE_STATUS_CHECK_FAILED), "{err}");
            assert!(out.contains("dx init"), "{out}");
        }
    }

    #[test]
    fn a_pin_mismatch_does_not_mask_other_findings() {
        let inv = invocation(&["status"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-status-both-");
        let root = scratch.path().to_path_buf();
        std::fs::create_dir_all(root.join(".dx")).expect("dx");
        std::fs::write(root.join(".dx/pins"), "9.9.9\n").expect("pin");
        let (code, out, err) = run(&inv, &root);
        assert_eq!(code, 1);
        assert!(err.contains(CODE_STATUS_CHECK_FAILED), "{err}");
        assert!(out.contains("pin: warn"), "{out}");
    }

    #[test]
    fn a_missing_managed_tool_fails_status() {
        let inv = invocation(&["status", "--output=json", "--capabilities"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-status-tool-");
        let root = scratch.path().to_path_buf();
        std::fs::create_dir_all(root.join(".dx/local/setups")).expect("setups");
        std::fs::write(root.join(".dx/pins"), "0.0.0\n").expect("pin");
        write_module(&root);
        std::fs::write(root.join(".dx/local/setups/current"), "foreign").expect("foreign");
        let (code, out, err) = run(&inv, &root);
        assert_eq!(code, 1);
        assert!(err.contains(CODE_STATUS_CHECK_FAILED), "{err}");
        let events = json_events(&out);
        let tools = events
            .iter()
            .find(|event| {
                event["event"] == serde_json::json!("status")
                    && event["name"] == serde_json::json!("tools")
            })
            .expect("tools check");
        assert_eq!(tools["severity"], serde_json::json!("error"));
        let record = events
            .iter()
            .find(|event| event["name"] == serde_json::json!("workspace:tools"))
            .expect("tools record");
        assert_eq!(record["availability"], serde_json::json!("unavailable"));
    }

    #[test]
    fn status_reports_capabilities_alongside_a_pin_warning() {
        let inv = invocation(&["status", "--output=json", "--capabilities"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-status-cap-fail-");
        let root = scratch.path().to_path_buf();
        std::fs::create_dir_all(root.join(".dx")).expect("dx");
        std::fs::write(root.join(".dx/pins"), "9.9.9\n").expect("pin");
        write_module(&root);
        let (code, out, _) = run(&inv, &root);
        assert_eq!(code, 0);
        let events = json_events(&out);
        let kinds = event_kinds(&events);
        assert!(kinds.contains(&"capability"), "{kinds:?}");
        assert_eq!(kinds[kinds.len() - 1], "command_finished");
    }

    struct BrokenPipeWriter;

    impl io::Write for BrokenPipeWriter {
        fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
            Err(io::Error::new(io::ErrorKind::BrokenPipe, "broken pipe"))
        }

        fn flush(&mut self) -> io::Result<()> {
            Err(io::Error::new(io::ErrorKind::BrokenPipe, "broken pipe"))
        }
    }

    #[test]
    fn status_broken_pipe_returns_141() {
        for words in [vec!["status"], vec!["status", "--output=json"]] {
            let inv = invocation(&words);
            let scratch = dx_test_scratch::scratch("dx-adopt-status-broken-");
            let root = scratch.path().to_path_buf();
            std::fs::create_dir_all(root.join(".dx")).expect("dx");
            std::fs::write(root.join(".dx/pins"), "0.0.0\n").expect("pin");
            write_module(&root);
            let mut err = Vec::new();
            let code = execute_status(&inv, &root, &mut BrokenPipeWriter, &mut err);
            assert_eq!(code, 128 + 13, "words: {words:?}");
        }
    }
}
