use std::io::Write;

use clap::ValueEnum;

use crate::args::{is_discovery_exempt, Command, Invocation};
use crate::exec::common::{check_stdout_write, emit_event, emit_started};
use dx_output::{
    capabilities_event, command_finished, command_started, status_event, CapabilitiesEntry,
    FinishedCounts, OutputMode, StatusEvent,
};

use super::{operational, summaries_suppressed};

pub(crate) const CODE_WORKSPACE_UNAVAILABLE: &str = "workspace_unavailable";

pub(crate) fn entry_for(command: Command) -> CapabilitiesEntry {
    let meta = command.meta();
    let mut flags: Vec<String> = crate::args::grammar::advertised_flags(command)
        .into_iter()
        .map(|flag| format!("--{flag}"))
        .collect();
    flags.sort();
    flags.dedup();
    let mut outputs = vec!["text".to_owned()];
    if meta.supports_diff {
        outputs.push("diff".to_owned());
    }
    if meta.supports_json {
        outputs.push("json".to_owned());
    }
    CapabilitiesEntry {
        name: meta.name.to_owned(),
        describe: meta.describe.to_owned(),
        usage: meta.usage.to_owned(),
        scope_policy: meta.scope_policy.to_owned(),
        flags,
        outputs,
        reports: crate::plan::spec(command)
            .reports
            .iter()
            .map(|format| format.name().to_owned())
            .collect(),
        supports_check: command.supports_check(),
        supports_apply: command.supports_apply(),
        mutating_by_default: command.is_mutating_by_default(),
        workspace_free: is_discovery_exempt(command),
        skew: meta.skew.name().to_owned(),
        labels: meta.labels.name().to_owned(),
    }
}

fn text_block(command: Command) -> String {
    let entry = entry_for(command);
    let reports = if entry.reports.is_empty() {
        "none".to_owned()
    } else {
        entry.reports.join(", ")
    };
    let yes = |flag: bool| if flag { "yes" } else { "no" };
    format!(
        "dx {} - {}\n  scopes: {}; output: {}; flags: {}; reports: {}; check: {}; apply: {}; workspace-free: {}; skew: {}",
        entry.name,
        entry.describe,
        entry.scope_policy,
        entry.outputs.join("|"),
        entry.flags.join(", "),
        reports,
        yes(entry.supports_check),
        yes(entry.supports_apply),
        yes(entry.workspace_free),
        entry.skew,
    )
}

fn availability_boundary() -> dx_adopt::StatusCheck {
    dx_adopt::StatusCheck {
        name: "availability".to_owned(),
        status: "warn".to_owned(),
        detail: "workspace facts come from local provenance records only; declared tool and policy availability is not probed"
            .to_owned(),
        hint: "run the owning command to observe availability".to_owned(),
    }
}

fn workspace_checks(workspace: &std::path::Path, pinned: &str) -> Vec<dx_adopt::StatusCheck> {
    let mut checks = vec![dx_adopt::StatusCheck {
        name: "workspace".to_owned(),
        status: "ok".to_owned(),
        detail: format!("selected workspace {}", workspace.display()),
        hint: "pass --workspace <dir> to select another workspace".to_owned(),
    }];
    checks.extend(dx_adopt::default_status_checks(pinned));
    checks.push(availability_boundary());
    checks
}

fn to_event(check: &dx_adopt::StatusCheck) -> StatusEvent {
    StatusEvent {
        name: check.name.clone(),
        status: check.status.clone(),
        detail: check.detail.clone(),
        hint: check.hint.clone(),
    }
}

pub(crate) fn execute_capabilities(
    invocation: &Invocation,
    workspace: &std::path::Path,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    if invocation.dry_run {
        if invocation.output == OutputMode::Json {
            if let Ok(event) = command_started(invocation.command.name(), true, "default") {
                if let Err(exit) = emit_event(out, &event) {
                    return exit;
                }
            }
            if let Err(exit) = emit_event(out, &command_finished(0, &FinishedCounts::default())) {
                return exit;
            }
        } else if !summaries_suppressed(invocation) {
            let plan = if invocation.workspace_capabilities {
                "would export capabilities with workspace facts"
            } else {
                "would export capabilities"
            };
            if let Err(exit) = check_stdout_write(writeln!(out, "{plan}")) {
                return exit;
            }
        }
        return 0;
    }
    if invocation.output == OutputMode::Json {
        if let Err(exit) = emit_started(invocation, out) {
            return exit;
        }
        for command in Command::value_variants() {
            match capabilities_event(&entry_for(*command)) {
                Ok(event) => {
                    if let Err(exit) = emit_event(out, &event) {
                        return exit;
                    }
                }
                Err(error) => {
                    return operational(
                        invocation,
                        out,
                        err,
                        CODE_WORKSPACE_UNAVAILABLE,
                        &error.to_string(),
                    );
                }
            }
        }
    } else if !summaries_suppressed(invocation) {
        for command in Command::value_variants() {
            if let Err(exit) = check_stdout_write(writeln!(out, "{}", text_block(*command))) {
                return exit;
            }
        }
    }
    if !invocation.workspace_capabilities {
        if invocation.output == OutputMode::Json {
            if let Err(exit) = emit_event(out, &command_finished(0, &FinishedCounts::default())) {
                return exit;
            }
        }
        return 0;
    }
    let pinned = match dx_adopt::read_version_pin(workspace) {
        Ok(pin) => pin,
        Err(error) => {
            return operational(
                invocation,
                out,
                err,
                CODE_WORKSPACE_UNAVAILABLE,
                &format!("cannot read workspace capabilities: {error}"),
            );
        }
    };
    let checks = workspace_checks(workspace, &pinned);
    if invocation.output == OutputMode::Json {
        for check in &checks {
            if let Ok(event) = status_event(&to_event(check)) {
                if let Err(exit) = emit_event(out, &event) {
                    return exit;
                }
            }
        }
    } else if !summaries_suppressed(invocation) {
        if let Err(exit) = check_stdout_write(writeln!(
            out,
            "workspace: {}\n{}",
            workspace.display(),
            dx_adopt::render_status_text(&checks)
        )) {
            return exit;
        }
    }
    if checks.iter().any(|check| check.status == "error") {
        return operational(
            invocation,
            out,
            err,
            CODE_WORKSPACE_UNAVAILABLE,
            "workspace pin mismatch (see hint)",
        );
    }
    if invocation.output == OutputMode::Json {
        if let Err(exit) = emit_event(out, &command_finished(0, &FinishedCounts::default())) {
            return exit;
        }
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adopt::test_support::{event_kinds, invocation, json_events, run};

    fn pin(root: &std::path::Path, version: &str) {
        std::fs::create_dir_all(root.join(".dx")).expect("dx dir");
        std::fs::write(root.join(".dx/version"), format!("{version}\n")).expect("pin");
    }

    #[test]
    fn entries_derive_every_command_from_the_registry() {
        for command in Command::value_variants() {
            let entry = entry_for(*command);
            assert_eq!(entry.name, command.name());
            let event = capabilities_event(&entry).expect("entry renders");
            assert_eq!(event["event"], serde_json::json!("capabilities"));
            assert!(event.get("schema").is_some(), "{event}");
        }
        let lint = entry_for(Command::Lint);
        assert!(lint.flags.contains(&"--check".to_owned()));
        assert!(lint.flags.contains(&"--apply".to_owned()));
        assert_eq!(lint.outputs, vec!["text", "diff", "json"]);
        assert_eq!(lint.reports, vec!["sarif"]);
        assert!(lint.supports_check && lint.supports_apply);
        assert!(!lint.workspace_free);
        let capabilities = entry_for(Command::Capabilities);
        assert!(capabilities
            .flags
            .contains(&"--workspace-capabilities".to_owned()));
        assert!(!capabilities.flags.contains(&"--check".to_owned()));
        assert!(!capabilities.flags.contains(&"--apply".to_owned()));
        assert_eq!(capabilities.outputs, vec!["text", "json"]);
        assert!(capabilities.reports.is_empty());
        assert!(capabilities.workspace_free);
        assert!(!capabilities.supports_check && !capabilities.supports_apply);
        assert!(!capabilities.mutating_by_default);
        assert!(entry_for(Command::Completion).workspace_free);
        assert!(!entry_for(Command::Build).workspace_free);
    }

    #[test]
    fn text_lists_every_command_with_its_effect_flags() {
        let inv = invocation(&["capabilities"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-capabilities-text-");
        let (code, out, err) = run(&inv, scratch.path());
        assert_eq!(code, 0, "{err}");
        assert!(err.is_empty(), "{err}");
        for command in Command::value_variants() {
            assert!(
                out.contains(&format!("dx {} - ", command.name())),
                "missing dx {} in:\n{out}",
                command.name()
            );
        }
        assert!(out.contains("--workspace-capabilities"), "{out}");
    }

    #[test]
    fn json_streams_one_capabilities_event_per_command() {
        let inv = invocation(&["capabilities", "--output=json"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-capabilities-json-");
        let (code, out, err) = run(&inv, scratch.path());
        assert_eq!(code, 0, "{err}");
        assert!(err.is_empty(), "{err}");
        let events = json_events(&out);
        let kinds = event_kinds(&events);
        assert_eq!(kinds[0], "command_started");
        assert_eq!(kinds[kinds.len() - 1], "command_finished");
        let entries: Vec<&serde_json::Value> = events
            .iter()
            .filter(|event| event["event"] == serde_json::json!("capabilities"))
            .collect();
        assert_eq!(entries.len(), Command::value_variants().len());
        for event in &entries {
            assert!(event.get("schema").is_some(), "{event}");
            assert_eq!(
                event["schema"]["major"],
                serde_json::json!(dx_output::SCHEMA_MAJOR),
                "{event}"
            );
            let name = event["name"].as_str().expect("name");
            assert!(Command::parse(name).is_some(), "{event}");
        }
        assert_eq!(
            events.last().expect("finished")["exit_code"],
            serde_json::json!(0)
        );
    }

    #[test]
    fn workspace_stage_reports_local_provenance() {
        let inv = invocation(&["capabilities", "--workspace-capabilities"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-capabilities-ws-");
        let root = scratch.path();
        pin(root, "0.0.0");
        let (code, out, err) = run(&inv, root);
        assert_eq!(code, 0, "{err}");
        assert!(err.is_empty(), "{err}");
        assert!(out.contains("dx capabilities - "), "{out}");
        assert!(out.contains("workspace: "), "{out}");
        assert!(out.contains("pin: ok"), "{out}");
        assert!(out.contains("availability: warn"), "{out}");
    }

    #[test]
    fn workspace_stage_json_keeps_declared_before_observed() {
        let inv = invocation(&["capabilities", "--workspace-capabilities", "--output=json"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-capabilities-ws-json-");
        let root = scratch.path();
        pin(root, "0.0.0");
        let (code, out, err) = run(&inv, root);
        assert_eq!(code, 0, "{err}");
        assert!(err.is_empty(), "{err}");
        let events = json_events(&out);
        let kinds = event_kinds(&events);
        assert_eq!(kinds[0], "command_started");
        assert_eq!(kinds[kinds.len() - 1], "command_finished");
        let first_status = kinds
            .iter()
            .position(|kind| *kind == "status")
            .expect("workspace facts");
        assert!(
            kinds[..first_status]
                .iter()
                .all(|kind| *kind == "command_started" || *kind == "capabilities"),
            "{kinds:?}"
        );
        let names: Vec<&str> = events
            .iter()
            .filter(|event| event["event"] == serde_json::json!("status"))
            .map(|event| event["name"].as_str().expect("status name"))
            .collect();
        assert_eq!(
            names,
            vec![
                "workspace",
                "toolchain",
                "platform",
                "tools",
                "pin",
                "availability"
            ],
            "{names:?}"
        );
        let pin = events
            .iter()
            .find(|event| {
                event["event"] == serde_json::json!("status")
                    && event["name"] == serde_json::json!("pin")
            })
            .expect("pin status");
        assert_eq!(pin["status"], serde_json::json!("ok"));
        let availability = events
            .iter()
            .find(|event| {
                event["event"] == serde_json::json!("status")
                    && event["name"] == serde_json::json!("availability")
            })
            .expect("availability boundary");
        assert_eq!(availability["status"], serde_json::json!("warn"));
        assert_eq!(
            events.last().expect("finished")["exit_code"],
            serde_json::json!(0)
        );
    }

    #[test]
    fn workspace_stage_fails_closed_on_pin_mismatch() {
        for words in [
            vec!["capabilities", "--workspace-capabilities"],
            vec!["capabilities", "--workspace-capabilities", "--output=json"],
        ] {
            let inv = invocation(&words);
            let scratch = dx_test_scratch::scratch("dx-adopt-capabilities-mismatch-");
            let root = scratch.path();
            pin(root, "9.9.9");
            let (code, out, err) = run(&inv, root);
            assert_eq!(code, 1, "words: {words:?}");
            assert!(
                err.contains("workspace_unavailable"),
                "words: {words:?}: {err}"
            );
            if words.contains(&"--output=json") {
                assert!(
                    out.contains("\"name\":\"capabilities\""),
                    "words: {words:?}"
                );
                let events = json_events(&out);
                let kinds = event_kinds(&events);
                assert_eq!(kinds[kinds.len() - 1], "command_finished");
                assert_eq!(
                    events.last().expect("finished")["exit_code"],
                    serde_json::json!(1)
                );
                let error = events
                    .iter()
                    .find(|event| event["event"] == serde_json::json!("error"))
                    .expect("explicit error");
                assert_eq!(error["code"], serde_json::json!("workspace_unavailable"));
            } else {
                assert!(
                    out.contains("dx capabilities - "),
                    "words: {words:?}: {out}"
                );
                assert!(out.contains("pin: error"), "words: {words:?}: {out}");
            }
        }
    }

    #[test]
    fn workspace_stage_fails_closed_without_a_pin() {
        for words in [
            vec!["capabilities", "--workspace-capabilities"],
            vec!["capabilities", "--workspace-capabilities", "--output=json"],
        ] {
            let inv = invocation(&words);
            let scratch = dx_test_scratch::scratch("dx-adopt-capabilities-nopin-");
            let root = scratch.path();
            let (code, out, err) = run(&inv, root);
            assert_eq!(code, 1, "words: {words:?}");
            assert!(
                err.contains("workspace_unavailable"),
                "words: {words:?}: {err}"
            );
            if words.contains(&"--output=json") {
                assert!(
                    out.contains("\"name\":\"capabilities\""),
                    "words: {words:?}"
                );
                let events = json_events(&out);
                assert_eq!(
                    events.last().expect("finished")["exit_code"],
                    serde_json::json!(1)
                );
            } else {
                assert!(
                    out.contains("dx capabilities - "),
                    "words: {words:?}: {out}"
                );
            }
        }
    }

    #[test]
    fn dry_run_plans_without_probing() {
        let scratch = dx_test_scratch::scratch("dx-adopt-capabilities-dry-");
        let root = scratch.path();
        let inv = invocation(&["capabilities", "--dry-run"]);
        let (code, out, _err) = run(&inv, root);
        assert_eq!(code, 0);
        assert!(out.contains("would export capabilities"));
        let inv = invocation(&["capabilities", "--workspace-capabilities", "--dry-run"]);
        let (code, out, _err) = run(&inv, root);
        assert_eq!(code, 0, "dry run never reads the missing pin");
        assert!(out.contains("would export capabilities with workspace facts"));
        let inv = invocation(&["capabilities", "--dry-run", "--quiet"]);
        let (code, out, _err) = run(&inv, root);
        assert_eq!(code, 0);
        assert!(out.is_empty());
        let inv = invocation(&["capabilities", "--dry-run", "--output=json"]);
        let (code, out, _err) = run(&inv, root);
        assert_eq!(code, 0);
        let events = json_events(&out);
        let kinds = event_kinds(&events);
        assert_eq!(kinds, vec!["command_started", "command_finished"]);
    }

    #[test]
    fn quiet_suppresses_summaries() {
        let inv = invocation(&["capabilities", "--quiet"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-capabilities-quiet-");
        let (code, out, err) = run(&inv, scratch.path());
        assert_eq!(code, 0, "{err}");
        assert!(out.is_empty());
        assert!(err.is_empty(), "{err}");
    }
}
