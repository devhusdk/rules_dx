use std::io::Write;

use clap::ValueEnum;

use crate::args::grammar::advertises_passthrough;
use crate::args::{Command, Invocation};
use crate::exec::common::{check_stdout_write, emit_built, emit_event, emit_started};
use dx_output::{
    capability_event, command_finished, command_started, status_event, CapabilityEvent,
    FinishedCounts, OutputMode, StatusEvent,
};

pub(crate) const CODE_STATUS_PIN_MISMATCH: &str = "status_pin_mismatch";

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
    if command.supports_diff() {
        modes.push("diff".to_owned());
    }
    if command.supports_json() {
        modes.push("json".to_owned());
    }
    modes
}

fn command_capability(command: Command) -> CapabilityEvent {
    let spec = crate::plan::spec(command);
    CapabilityEvent {
        name: command.name().to_owned(),
        source: "cli-grammar".to_owned(),
        availability: "available".to_owned(),
        detail: command.describe().to_owned(),
        flags: crate::args::grammar::advertised_flags(command)
            .into_iter()
            .map(|flag| format!("--{flag}"))
            .collect(),
        outputs: output_names(command),
        reports: crate::reports::format_names(spec.reports)
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
    if failed.contains(&"pin") {
        return Some((
            CODE_STATUS_PIN_MISMATCH,
            "pin mismatch (see hint)".to_owned(),
        ));
    }
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

fn agents_check(workspace: &std::path::Path) -> dx_adopt::StatusCheck {
    use super::agents::{workspace_state, State, FILE};
    let (mut status, mut detail) = match workspace_state(workspace) {
        Ok(State::Current) => ("ok", format!("{FILE} has the current dx block")),
        Ok(State::Stale) => ("warn", format!("{FILE} has an out-of-date dx block")),
        Ok(State::Missing) => ("warn", format!("{FILE} has no dx block")),
        Err(error) => ("warn", format!("{FILE} is unreadable: {error}")),
    };
    let missing = crate::worktree::setup::workspace_missing(workspace);
    if !missing.is_empty() {
        status = "warn";
        detail = format!(
            "{detail}; {} lacks hooks: {}",
            crate::worktree::setup::SETTINGS,
            missing.join("; ")
        );
    }
    dx_adopt::StatusCheck {
        name: "agents".to_owned(),
        status: status.to_owned(),
        detail,
        hint: "dx init --apply writes the block and hooks; dx upgrade --apply refreshes the block"
            .to_owned(),
    }
}

fn lease_check(workspace: &std::path::Path) -> Option<dx_adopt::StatusCheck> {
    let (path, record) = crate::worktree::lease_for_workspace(workspace)?;
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

pub(crate) fn execute_status(
    invocation: &Invocation,
    workspace: &std::path::Path,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let pinned = match dx_adopt::read_version_pin(workspace) {
        Ok(pin) => pin,
        Err(error) => {
            let message = error.to_string();
            if invocation.output == OutputMode::Json {
                if let Err(exit) = emit_started(invocation, out) {
                    return exit;
                }
            }
            return operational(invocation, out, err, CODE_STATUS_PIN_MISMATCH, &message);
        }
    };
    let tools = super::status_tools::managed_tools_check(workspace);
    let records = if invocation.verbose {
        capabilities(&tools)
    } else {
        Vec::new()
    };
    let mut checks = dx_adopt::observed_status_checks(workspace, &pinned, tools);
    checks.push(agents_check(workspace));
    if let Some(check) = lease_check(workspace) {
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
                    status: check.status.clone(),
                    detail: check.detail.clone(),
                    hint: check.hint.clone(),
                }),
            ) {
                return exit;
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
    for capability in &records {
        if let Err(exit) = check_stdout_write(writeln!(out, "{}", capability_line(capability))) {
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
    use crate::adopt::test_support::{event_kinds, invocation, json_events, run};
    use std::io;

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
        let record = crate::worktree::pool::Record {
            slot: 3,
            held: true,
            branch: Some("feature/a".to_owned()),
            last_branch: None,
            session: Some("s1".to_owned()),
            agent: None,
            pid: Some(7),
            gone: false,
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
    fn status_flags_a_missing_or_stale_agents_block_and_accepts_a_current_one() {
        let scratch = dx_test_scratch::scratch("dx-adopt-status-agents-");
        let root = scratch.path();
        let check = agents_check(root);
        assert_eq!(
            (check.name.as_str(), check.status.as_str()),
            ("agents", "warn")
        );
        assert!(check.detail.contains("no dx block"), "{}", check.detail);
        std::fs::write(
            root.join("AGENTS.md"),
            "# Mine\n<!-- dx:begin -->\n- old\n<!-- dx:end -->\n",
        )
        .expect("stale");
        let check = agents_check(root);
        assert_eq!(check.status, "warn");
        assert!(check.detail.contains("out-of-date"), "{}", check.detail);
        std::fs::write(root.join("AGENTS.md"), super::super::agents::block()).expect("current");
        let check = agents_check(root);
        assert_eq!(check.status, "warn");
        assert!(check.detail.contains("lacks hooks"), "{}", check.detail);
        std::fs::create_dir_all(root.join(".claude")).expect("dir");
        std::fs::write(
            root.join(".claude/settings.json"),
            crate::worktree::setup::settings_text(),
        )
        .expect("settings");
        assert_eq!(agents_check(root).status, "ok");
    }

    #[test]
    fn status_reports_pin_and_checks() {
        let inv = invocation(&["status"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-status-status-");
        let root = scratch.path().to_path_buf();
        std::fs::create_dir_all(root.join(".dx")).expect("dx");
        std::fs::write(root.join(".dx/version"), "0.0.0\n").expect("pin");
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
        std::fs::write(root.join(".dx/version"), "0.0.0\n").expect("pin");
        write_module(&root);
        dx_adopt::write_launcher_files(&root, "0.0.0").expect("launchers");
        let (code, out, _err) = run(&inv, &root);
        assert_eq!(code, 0, "{out}");
        assert!(out.contains("launcher: ok"), "{out}");
        dx_adopt::write_launcher_files(&root, "1.2.3").expect("stale");
        let (code, out, err) = run(&inv, &root);
        assert_eq!(code, 1, "{out}");
        assert!(
            out.contains("launcher: error (tools/dx pins dx 1.2.3"),
            "{out}"
        );
        assert!(err.contains("status_check_failed"), "{err}");
        assert!(err.contains("failed checks: launcher"), "{err}");
    }

    #[test]
    fn status_json_streams_envelope() {
        let inv = invocation(&["status", "--output=json"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-status-json-");
        let root = scratch.path().to_path_buf();
        std::fs::create_dir_all(root.join(".dx")).expect("dx");
        std::fs::write(root.join(".dx/version"), "0.0.0\n").expect("pin");
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
        assert_eq!(status["status"], serde_json::json!("ok"));
        assert!(err.is_empty());
    }

    #[test]
    fn status_pin_mismatch_fails_operational() {
        for words in [vec!["status"], vec!["status", "--output=json"]] {
            let inv = invocation(&words);
            let scratch = dx_test_scratch::scratch("dx-adopt-status-mismatch-");
            let root = scratch.path().to_path_buf();
            std::fs::create_dir_all(root.join(".dx")).expect("dx");
            std::fs::write(root.join(".dx/version"), "9.9.9\n").expect("pin");
            write_module(&root);
            let (code, out, err) = run(&inv, &root);
            assert_eq!(code, 1, "words: {words:?}");
            assert!(err.contains("pin mismatch"), "words: {words:?}");
            if words.contains(&"--output=json") {
                let text = out;
                let events = json_events(&text);
                assert_eq!(
                    events.last().expect("finished")["exit_code"],
                    serde_json::json!(1)
                );
                let kinds = event_kinds(&events);
                assert_eq!(kinds[0], "command_started");
                assert_eq!(kinds[kinds.len() - 1], "command_finished");
                assert!(kinds.contains(&"status"), "{kinds:?}");
                let error_index = kinds
                    .iter()
                    .position(|kind| *kind == "error")
                    .expect("status failure emits error");
                assert_eq!(kinds[error_index + 1..], ["command_finished"]);
                let error = &events[error_index];
                assert_eq!(error["code"], serde_json::json!("status_pin_mismatch"));
            }
        }
    }

    #[test]
    fn status_missing_pin_fails_closed() {
        for words in [vec!["status"], vec!["status", "--output=json"]] {
            let inv = invocation(&words);
            let scratch = dx_test_scratch::scratch("dx-adopt-status-missing-");
            let root = scratch.path().to_path_buf();
            let (code, out, err) = run(&inv, &root);
            assert_eq!(code, 1, "words: {words:?}");
            assert!(err.contains("read version pin"), "words: {words:?}");
            let stdout = out;
            assert!(!stdout.contains("pin: ok"), "words: {words:?}");
            if words.contains(&"--output=json") {
                let events = json_events(&stdout);
                assert_eq!(events[0]["event"], serde_json::json!("command_started"));
                assert_eq!(
                    events.last().expect("finished")["exit_code"],
                    serde_json::json!(1)
                );
                let kinds = event_kinds(&events);
                assert_eq!(kinds[0], "command_started");
                assert_eq!(kinds[kinds.len() - 1], "command_finished");
                assert!(!kinds.contains(&"status"), "{kinds:?}");
                let error_index = kinds
                    .iter()
                    .position(|kind| *kind == "error")
                    .expect("missing pin emits error");
                assert_eq!(kinds[error_index + 1..], ["command_finished"]);
                assert_eq!(
                    events[error_index]["code"],
                    serde_json::json!("status_pin_mismatch")
                );
            }
        }
    }

    #[test]
    fn status_empty_pin_reports_error() {
        let inv = invocation(&["status"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-status-empty-");
        let root = scratch.path().to_path_buf();
        std::fs::create_dir_all(root.join(".dx")).expect("dx");
        std::fs::write(root.join(".dx/version"), "\n").expect("empty pin");
        write_module(&root);
        let (code, out, err) = run(&inv, &root);
        assert_eq!(code, 1);
        assert!(err.contains("pin mismatch"));
        assert!(!out.contains("pin: ok"));
    }

    #[test]
    fn status_text_lists_every_command_and_workspace_tools() {
        let inv = invocation(&["status", "--verbose"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-status-cap-text-");
        let root = scratch.path().to_path_buf();
        std::fs::create_dir_all(root.join(".dx")).expect("dx");
        std::fs::write(root.join(".dx/version"), "0.0.0\n").expect("pin");
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

    #[test]
    fn status_default_output_omits_capability_records() {
        let scratch = dx_test_scratch::scratch("dx-adopt-status-compact-");
        let root = scratch.path().to_path_buf();
        std::fs::create_dir_all(root.join(".dx")).expect("dx");
        std::fs::write(root.join(".dx/version"), "0.0.0\n").expect("pin");
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
    fn status_json_streams_capability_records() {
        let inv = invocation(&["status", "--output=json", "--verbose"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-status-cap-json-");
        let root = scratch.path().to_path_buf();
        std::fs::create_dir_all(root.join(".dx")).expect("dx");
        std::fs::write(root.join(".dx/version"), "0.0.0\n").expect("pin");
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
            ("fix", "apply"),
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
        std::fs::write(root.join(".dx/version"), "0.0.0\n").expect("pin");
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
        assert_eq!(module["status"], serde_json::json!("error"));
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
    fn health_findings_never_report_a_pin_mismatch() {
        for words in [vec!["status"], vec!["status", "--output=json"]] {
            let inv = invocation(&words);
            let scratch = dx_test_scratch::scratch("dx-adopt-status-bare-");
            let root = scratch.path().to_path_buf();
            std::fs::create_dir_all(root.join(".dx")).expect("dx");
            std::fs::write(root.join(".dx/version"), "0.0.0\n").expect("pin");
            std::fs::write(root.join("MODULE.bazel"), "module(name = \"app\")\n").expect("module");
            let (code, out, err) = run(&inv, &root);
            assert_eq!(code, 1, "words: {words:?}");
            assert!(err.contains(CODE_STATUS_CHECK_FAILED), "{err}");
            assert!(!err.contains(CODE_STATUS_PIN_MISMATCH), "{err}");
            assert!(out.contains("dx init"), "{out}");
        }
    }

    #[test]
    fn a_real_pin_mismatch_wins_over_other_findings() {
        let inv = invocation(&["status"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-status-both-");
        let root = scratch.path().to_path_buf();
        std::fs::create_dir_all(root.join(".dx")).expect("dx");
        std::fs::write(root.join(".dx/version"), "9.9.9\n").expect("pin");
        let (code, _, err) = run(&inv, &root);
        assert_eq!(code, 1);
        assert!(err.contains(CODE_STATUS_PIN_MISMATCH), "{err}");
    }

    #[test]
    fn a_missing_managed_tool_fails_status() {
        let inv = invocation(&["status", "--output=json", "--verbose"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-status-tool-");
        let root = scratch.path().to_path_buf();
        std::fs::create_dir_all(root.join(".dx/setups")).expect("setups");
        std::fs::write(root.join(".dx/version"), "0.0.0\n").expect("pin");
        write_module(&root);
        std::fs::write(root.join(".dx/setups/current"), "foreign").expect("foreign");
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
        assert_eq!(tools["status"], serde_json::json!("error"));
        let record = events
            .iter()
            .find(|event| event["name"] == serde_json::json!("workspace:tools"))
            .expect("tools record");
        assert_eq!(record["availability"], serde_json::json!("unavailable"));
    }

    #[test]
    fn status_reports_capabilities_alongside_a_pin_failure() {
        let inv = invocation(&["status", "--output=json", "--verbose"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-status-cap-fail-");
        let root = scratch.path().to_path_buf();
        std::fs::create_dir_all(root.join(".dx")).expect("dx");
        std::fs::write(root.join(".dx/version"), "9.9.9\n").expect("pin");
        write_module(&root);
        let (code, out, _) = run(&inv, &root);
        assert_eq!(code, 1);
        let events = json_events(&out);
        let kinds = event_kinds(&events);
        assert!(kinds.contains(&"capability"), "{kinds:?}");
        assert_eq!(kinds[kinds.len() - 1], "command_finished");
    }

    #[test]
    fn capabilities_command_is_rejected_with_status_guidance() {
        let err = crate::args::parse(&crate::test_support::strings(&["capabilities"]))
            .expect_err("removed command");
        match err {
            crate::args::ArgsError::Usage { text } => {
                assert!(
                    text.contains("unsupported command 'capabilities'"),
                    "{text}"
                );
                assert!(text.contains("dx status --verbose"), "{text}");
            }
            other => panic!("{other:?}"),
        }
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
            std::fs::write(root.join(".dx/version"), "0.0.0\n").expect("pin");
            write_module(&root);
            let mut err = Vec::new();
            let code = execute_status(&inv, &root, &mut BrokenPipeWriter, &mut err);
            assert_eq!(code, 128 + 13, "words: {words:?}");
        }
    }
}
