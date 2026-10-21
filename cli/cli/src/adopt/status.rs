use std::io::Write;

use clap::ValueEnum;

use crate::args::help::advertises_passthrough;
use crate::args::{Command, Invocation};
use crate::exec::common::{check_stdout_write, emit_event, emit_started};
use dx_output::{
    capability_event, command_finished, command_started, status_event, CapabilityEvent,
    FinishedCounts, OutputMode, StatusEvent,
};

pub(crate) const CODE_STATUS_PIN_MISMATCH: &str = "status_pin_mismatch";

use super::operational;

fn effect_of(command: Command) -> &'static str {
    if command.is_mutating_by_default() {
        "mutating"
    } else if command.supports_apply() {
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

fn unobserved_tools() -> CapabilityEvent {
    CapabilityEvent {
        name: "workspace:tools".to_owned(),
        source: "unobserved".to_owned(),
        availability: "unknown".to_owned(),
        detail: "tool availability is not probed by status".to_owned(),
        flags: Vec::new(),
        outputs: Vec::new(),
        reports: Vec::new(),
        scope_policy: "reject".to_owned(),
        effect: "none".to_owned(),
        passthrough: false,
    }
}

fn capabilities() -> Vec<CapabilityEvent> {
    let mut records: Vec<CapabilityEvent> = Command::value_variants()
        .iter()
        .map(|command| command_capability(*command))
        .collect();
    records.push(unobserved_tools());
    records
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
    let checks = dx_adopt::default_status_checks(&pinned);
    if invocation.output == OutputMode::Json {
        if let Ok(event) = command_started(invocation.command.name(), "default") {
            if let Err(exit) = emit_event(out, &event) {
                return exit;
            }
        }
        for check in &checks {
            if let Ok(event) = status_event(&StatusEvent {
                name: check.name.clone(),
                status: check.status.clone(),
                detail: check.detail.clone(),
                hint: check.hint.clone(),
            }) {
                if let Err(exit) = emit_event(out, &event) {
                    return exit;
                }
            }
        }
        for capability in &capabilities() {
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
        let failed = checks.iter().any(|c| c.status == "error");
        if failed {
            return operational(
                invocation,
                out,
                err,
                CODE_STATUS_PIN_MISMATCH,
                "pin mismatch (see hint)",
            );
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
    for capability in &capabilities() {
        if let Err(exit) = check_stdout_write(writeln!(out, "{}", capability_line(capability))) {
            return exit;
        }
    }
    if checks.iter().any(|c| c.status == "error") {
        return operational(
            invocation,
            out,
            err,
            CODE_STATUS_PIN_MISMATCH,
            "pin mismatch (see hint)",
        );
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adopt::test_support::{event_kinds, invocation, json_events, run};
    use std::io;

    #[test]
    fn status_reports_pin_and_checks() {
        let inv = invocation(&["status"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-status-status-");
        let root = scratch.path().to_path_buf();
        std::fs::create_dir_all(root.join(".dx")).expect("dx");
        std::fs::write(root.join(".dx/version"), "0.0.0\n").expect("pin");
        let (code, out, _err) = run(&inv, &root);
        assert_eq!(code, 0);
        assert!(out.contains("pin: ok"));
    }

    #[test]
    fn status_json_streams_envelope() {
        let inv = invocation(&["status", "--output=json"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-status-json-");
        let root = scratch.path().to_path_buf();
        std::fs::create_dir_all(root.join(".dx")).expect("dx");
        std::fs::write(root.join(".dx/version"), "0.0.0\n").expect("pin");
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
        let (code, out, err) = run(&inv, &root);
        assert_eq!(code, 1);
        assert!(err.contains("pin mismatch"));
        assert!(!out.contains("pin: ok"));
    }

    #[test]
    fn status_text_lists_every_command_and_unobserved_tools() {
        let inv = invocation(&["status"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-status-cap-text-");
        let root = scratch.path().to_path_buf();
        std::fs::create_dir_all(root.join(".dx")).expect("dx");
        std::fs::write(root.join(".dx/version"), "0.0.0\n").expect("pin");
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
    fn status_json_streams_capability_records() {
        let inv = invocation(&["status", "--output=json"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-status-cap-json-");
        let root = scratch.path().to_path_buf();
        std::fs::create_dir_all(root.join(".dx")).expect("dx");
        std::fs::write(root.join(".dx/version"), "0.0.0\n").expect("pin");
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
        assert_eq!(lint["availability"], serde_json::json!("available"));
        let tools = capabilities
            .iter()
            .find(|event| event["name"] == serde_json::json!("workspace:tools"))
            .expect("tools record");
        assert_eq!(tools["availability"], serde_json::json!("unknown"));
    }

    #[test]
    fn status_reports_capabilities_alongside_a_pin_failure() {
        let inv = invocation(&["status", "--output=json"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-status-cap-fail-");
        let root = scratch.path().to_path_buf();
        std::fs::create_dir_all(root.join(".dx")).expect("dx");
        std::fs::write(root.join(".dx/version"), "9.9.9\n").expect("pin");
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
                assert!(text.contains("dx status"), "{text}");
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
            let mut err = Vec::new();
            let code = execute_status(&inv, &root, &mut BrokenPipeWriter, &mut err);
            assert_eq!(code, 128 + 13, "words: {words:?}");
        }
    }
}
