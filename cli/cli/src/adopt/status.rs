use std::io::Write;

use crate::args::Invocation;
use crate::exec::common::{check_stdout_write, emit_event, emit_started};
use dx_output::{
    command_finished, command_started, status_event, FinishedCounts, OutputMode, StatusEvent,
};

pub(crate) const CODE_STATUS_PIN_MISMATCH: &str = "status_pin_mismatch";

use super::{operational, summaries_suppressed};

pub(crate) fn execute_status(
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
            if let Err(exit) = check_stdout_write(writeln!(out, "would report status")) {
                return exit;
            }
        }
        return 0;
    }
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
    let mut checks = dx_adopt::default_status_checks(&pinned);
    checks.push(dx_adopt::config_status_check(
        &dx_adopt::defaults::discover(workspace),
    ));
    if invocation.output == OutputMode::Json {
        if let Ok(event) = command_started(invocation.command.name(), false, "default") {
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
    fn status_config_check_discloses_the_effective_origin() {
        let inv = invocation(&["status"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-status-config-");
        let root = scratch.path().to_path_buf();
        std::fs::create_dir_all(root.join(".dx")).expect("dx");
        std::fs::write(root.join(".dx/version"), "0.0.0\n").expect("pin");
        let (code, out, _err) = run(&inv, &root);
        assert_eq!(code, 0);
        assert!(out.contains("config: ok"), "{out}");
        assert!(out.contains("built-in"), "{out}");
        std::fs::write(root.join("dx.toml"), "[dx]\nquiet = true\n").expect("committed");
        let (code, out, _err) = run(&inv, &root);
        assert_eq!(code, 0);
        assert!(out.contains("config: ok"), "{out}");
        assert!(out.contains("dx.toml"), "{out}");
        std::fs::write(root.join("dx.local.toml"), "[dx]\nquiet = false\n").expect("local");
        let (code, out, _err) = run(&inv, &root);
        assert_eq!(code, 0);
        assert!(out.contains("dx.local.toml"), "{out}");
        assert!(out.contains("local wins"), "{out}");
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
            events[0]["dry_run"],
            serde_json::Value::Bool(false),
            "{events:?}"
        );
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
    fn status_dry_run_plans_without_executing() {
        let inv = invocation(&["status", "--dry-run"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-status-dry-");
        let root = scratch.path().to_path_buf();
        let (code, out, _err) = run(&inv, &root);
        assert_eq!(code, 0);
        assert!(out.contains("would report status"));
        let inv = invocation(&["status", "--dry-run", "--quiet"]);
        let (code, out, _err) = run(&inv, &root);
        assert_eq!(code, 0);
        assert!(out.is_empty());
    }

    #[test]
    fn status_dry_run_json_emits_lifecycle_only() {
        let inv = invocation(&["status", "--dry-run", "--output=json"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-status-dry-json-");
        let root = scratch.path().to_path_buf();
        let (code, out, _err) = run(&inv, &root);
        assert_eq!(code, 0);
        let text = out;
        let events = json_events(&text);
        let kinds = event_kinds(&events);
        assert_eq!(kinds, vec!["command_started", "command_finished"]);
        assert_eq!(events[0]["dry_run"], serde_json::json!(true));
        assert_eq!(
            events.last().expect("finished")["exit_code"],
            serde_json::json!(0)
        );
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
