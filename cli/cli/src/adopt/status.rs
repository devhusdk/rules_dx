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
    ci: bool,
    allow_local: bool,
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
    if let Ok(loaded) = dx_adopt::defaults::load_defaults_with(workspace, !ci || allow_local) {
        checks.push(dx_adopt::config_status_check(&loaded));
    }
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

    fn seed_pin(root: &std::path::Path) {
        std::fs::create_dir_all(root.join(".dx")).expect("dx");
        std::fs::write(root.join(".dx/version"), "0.0.0\n").expect("pin");
    }

    #[test]
    fn status_discloses_committed_and_local_config_origin() {
        let inv = invocation(&["status"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-status-config-");
        let root = scratch.path().to_path_buf();
        seed_pin(&root);
        std::fs::write(root.join("dx.toml"), "[dx]\noutput = \"json\"\n").expect("committed");
        std::fs::write(root.join("dx.local.toml"), "[dx]\nquiet = true\n").expect("local");
        let before = std::fs::read(root.join("dx.toml")).expect("read committed");
        let (code, out, _err) = run(&inv, &root);
        assert_eq!(code, 0);
        assert!(
            out.contains("config: ok (defaults from dx.toml + dx.local.toml)"),
            "{out}"
        );
        assert_eq!(
            std::fs::read(root.join("dx.toml")).expect("reread committed"),
            before,
            "status never rewrites config"
        );
        assert!(
            !root.join(".dx/config.toml").exists(),
            "status creates no legacy config"
        );
    }

    #[test]
    fn status_config_row_covers_legacy_and_builtin_origins() {
        let inv = invocation(&["status"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-status-legacy-");
        let root = scratch.path().to_path_buf();
        seed_pin(&root);
        std::fs::create_dir_all(root.join(".dx")).expect("dx");
        std::fs::write(root.join(".dx/config.toml"), "[dx]\noutput = \"json\"\n")
            .expect("legacy");
        let (code, out, _err) = run(&inv, &root);
        assert_eq!(code, 0);
        assert!(
            out.contains("legacy defaults from"),
            "{out}"
        );
        assert!(out.contains(".dx/config.toml"), "{out}");
        let plain = dx_test_scratch::scratch("dx-adopt-status-builtin-");
        let bare = plain.path().to_path_buf();
        seed_pin(&bare);
        let (code, out, _err) = run(&inv, &bare);
        assert_eq!(code, 0);
        assert!(out.contains("built-in defaults (no config file)"), "{out}");
    }

    #[test]
    fn status_json_carries_the_config_check() {
        let inv = invocation(&["status", "--output=json"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-status-config-json-");
        let root = scratch.path().to_path_buf();
        seed_pin(&root);
        std::fs::write(root.join("dx.toml"), "[dx]\noutput = \"json\"\n").expect("committed");
        let (code, out, err) = run(&inv, &root);
        assert_eq!(code, 0, "{out}{err}");
        let events = json_events(&out);
        let config = events
            .iter()
            .find(|event| {
                event["event"] == serde_json::json!("status")
                    && event["name"] == serde_json::json!("config")
            })
            .expect("config status event");
        assert_eq!(config["status"], serde_json::json!("ok"));
        assert!(
            config["detail"].as_str().expect("detail").contains("dx.toml"),
            "{config}"
        );
    }

    #[test]
    fn status_under_ci_ignores_the_local_file() {
        let scratch = dx_test_scratch::scratch("dx-adopt-status-ci-");
        let root = scratch.path().to_path_buf();
        seed_pin(&root);
        std::fs::write(root.join("dx.toml"), "[dx]\noutput = \"json\"\n").expect("committed");
        std::fs::write(root.join("dx.local.toml"), "[dx]\noutput = \"text\"\n").expect("local");
        let inv = invocation(&["status"]);
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = execute_status(&inv, &root, &mut out, &mut err, true, false);
        assert_eq!(code, 0, "{}", String::from_utf8_lossy(&err));
        let text = String::from_utf8(out).expect("stdout");
        assert!(text.contains("defaults from dx.toml"), "{text}");
        assert!(text.contains("dx.local.toml ignored"), "{text}");
        assert!(text.contains("DX_ALLOW_LOCAL_CONFIG=1"), "{text}");
    }

    #[test]
    fn disposable_state_rebuild_keeps_config_behavior() {
        let status = invocation(&["status"]);
        let pin = invocation(&["version", "--pin=0.0.0"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-status-rebuild-");
        let root = scratch.path().to_path_buf();
        std::fs::write(
            root.join("dx.toml"),
            "[dx]\noutput = \"json\"\nquiet = true\n",
        )
        .expect("committed");
        let (code, _, _) = run(&pin, &root);
        assert_eq!(code, 0);
        let (code, before, _) = run(&status, &root);
        assert_eq!(code, 0);
        assert!(before.contains("defaults from dx.toml"), "{before}");
        let loaded_before =
            dx_adopt::defaults::load_defaults(&root).expect("defaults before deletion");
        assert_eq!(loaded_before.defaults.quiet, Some(true));
        std::fs::remove_dir_all(root.join(".dx")).expect("delete disposable state");
        assert!(!root.join(".dx").exists());
        let loaded_after =
            dx_adopt::defaults::load_defaults(&root).expect("defaults without .dx");
        assert_eq!(
            loaded_after.defaults, loaded_before.defaults,
            "deleting .dx keeps the effective defaults"
        );
        let (code, _, _) = run(&pin, &root);
        assert_eq!(code, 0, "the pin rebuilds with an explicit apply");
        let (code, after, _) = run(&status, &root);
        assert_eq!(code, 0);
        assert!(after.contains("defaults from dx.toml"), "{after}");
        for line in before.lines() {
            if line.starts_with("config:") {
                assert!(after.contains(line), "the config row survives: {after}");
            }
        }
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
            let code = execute_status(&inv, &root, &mut BrokenPipeWriter, &mut err, false, true);
            assert_eq!(code, 128 + 13, "words: {words:?}");
        }
    }
}
