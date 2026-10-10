use std::io::Write;

use crate::args::Invocation;
use crate::exec::common::{check_stdout_write, emit_event, emit_started};
use dx_output::{
    command_finished, command_started, status_event, FinishedCounts, OutputMode, StatusEvent,
};

use super::status::CODE_STATUS_PIN_MISMATCH;
use super::{operational, pre_exec, summaries_suppressed};

fn json_dry_run(invocation: &Invocation, out: &mut dyn Write) -> i32 {
    if let Ok(event) = command_started(invocation.command.name(), true, "default") {
        if let Err(exit) = emit_event(out, &event) {
            return exit;
        }
    }
    if let Err(exit) = emit_event(out, &command_finished(0, &FinishedCounts::default())) {
        return exit;
    }
    0
}

fn json_status(
    out: &mut dyn Write,
    name: &str,
    status: &str,
    detail: &str,
    hint: &str,
) -> Result<(), i32> {
    if let Ok(event) = status_event(&StatusEvent {
        name: name.to_owned(),
        status: status.to_owned(),
        detail: detail.to_owned(),
        hint: hint.to_owned(),
    }) {
        emit_event(out, &event)?;
    }
    Ok(())
}

pub(crate) fn execute_version(
    invocation: &Invocation,
    workspace: &std::path::Path,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    if invocation.check && (invocation.pin.is_some() || invocation.rollback) {
        return pre_exec(
            err,
            "version --check does not combine with --pin or --rollback",
        );
    }
    if invocation.pin.is_some() && invocation.rollback {
        return pre_exec(err, "version --pin and --rollback are mutually exclusive");
    }
    let is_json = invocation.output == OutputMode::Json;
    if invocation.rollback {
        let previous = dx_adopt::PREVIOUS_VERSION;
        let current = match dx_adopt::read_version_pin(workspace) {
            Ok(pin) => pin,
            Err(error) => {
                let message = error.to_string();
                if is_json {
                    if let Err(exit) = emit_started(invocation, out) {
                        return exit;
                    }
                }
                return operational(invocation, out, err, CODE_STATUS_PIN_MISMATCH, &message);
            }
        };
        if current.is_empty() || !dx_adopt::rollback_re_pins_previous(&current, previous, previous)
        {
            let message = format!(
                "rollback refused: pin {current:?} is not newer than previous release {previous:?}"
            );
            if is_json {
                if let Err(exit) = emit_started(invocation, out) {
                    return exit;
                }
            }
            return operational(invocation, out, err, CODE_STATUS_PIN_MISMATCH, &message);
        }
        if invocation.dry_run {
            if is_json {
                return json_dry_run(invocation, out);
            }
            if !summaries_suppressed(invocation) {
                if let Err(exit) =
                    check_stdout_write(writeln!(out, "would pin {previous} (rollback)"))
                {
                    return exit;
                }
            }
            return 0;
        }
        if !invocation.applies() {
            if current == previous {
                if is_json {
                    if let Ok(event) = command_started(invocation.command.name(), false, "check") {
                        if let Err(exit) = emit_event(out, &event) {
                            return exit;
                        }
                    }
                    if let Err(exit) = json_status(
                        out,
                        "pin",
                        "ok",
                        previous,
                        &format!("dx version --pin {}", dx_adopt::MODULE_VERSION),
                    ) {
                        return exit;
                    }
                    if let Err(exit) =
                        emit_event(out, &command_finished(0, &FinishedCounts::default()))
                    {
                        return exit;
                    }
                    return 0;
                }
                if !summaries_suppressed(invocation) {
                    if let Err(exit) =
                        check_stdout_write(writeln!(out, "version current: rollback {previous}"))
                    {
                        return exit;
                    }
                }
                return 0;
            }
            if is_json {
                if let Err(exit) = emit_started(invocation, out) {
                    return exit;
                }
            }
            return operational(
                invocation,
                out,
                err,
                CODE_STATUS_PIN_MISMATCH,
                &format!(
                    "version drift: pin {current:?} needs rollback to {previous:?} (run `dx version --rollback --apply`)"
                ),
            );
        }
        return match dx_adopt::write_version_pin(workspace, previous) {
            Ok(()) => {
                if is_json {
                    if let Ok(event) = command_started(invocation.command.name(), false, "default")
                    {
                        if let Err(exit) = emit_event(out, &event) {
                            return exit;
                        }
                    }
                    if let Err(exit) = json_status(
                        out,
                        "pin",
                        "ok",
                        previous,
                        &format!("dx version --pin {}", dx_adopt::MODULE_VERSION),
                    ) {
                        return exit;
                    }
                    if let Err(exit) =
                        emit_event(out, &command_finished(0, &FinishedCounts::default()))
                    {
                        return exit;
                    }
                    return 0;
                }
                if let Err(exit) = check_stdout_write(writeln!(out, "pinned {previous} (rollback)"))
                {
                    return exit;
                }
                0
            }
            Err(error) => {
                let message = error.to_string();
                if is_json {
                    if let Err(exit) = emit_started(invocation, out) {
                        return exit;
                    }
                }
                operational(invocation, out, err, CODE_STATUS_PIN_MISMATCH, &message)
            }
        };
    }
    if let Some(pin) = &invocation.pin {
        if !dx_adopt::version_pin_matches_module(pin, dx_adopt::MODULE_VERSION) {
            let message = format!("version pin must equal module {}", dx_adopt::MODULE_VERSION);
            if is_json {
                if let Err(exit) = emit_started(invocation, out) {
                    return exit;
                }
            }
            return operational(invocation, out, err, CODE_STATUS_PIN_MISMATCH, &message);
        }
        if invocation.dry_run {
            if is_json {
                return json_dry_run(invocation, out);
            }
            if !summaries_suppressed(invocation) {
                if let Err(exit) = check_stdout_write(writeln!(out, "would pin {pin}")) {
                    return exit;
                }
            }
            return 0;
        }
        if !invocation.applies() {
            let current = dx_adopt::read_version_pin(workspace).unwrap_or_default();
            if current == *pin {
                if is_json {
                    if let Ok(event) = command_started(invocation.command.name(), false, "check") {
                        if let Err(exit) = emit_event(out, &event) {
                            return exit;
                        }
                    }
                    if let Err(exit) = json_status(
                        out,
                        "pin",
                        "ok",
                        pin,
                        &format!("dx version --pin {}", dx_adopt::MODULE_VERSION),
                    ) {
                        return exit;
                    }
                    if let Err(exit) =
                        emit_event(out, &command_finished(0, &FinishedCounts::default()))
                    {
                        return exit;
                    }
                    return 0;
                }
                if !summaries_suppressed(invocation) {
                    if let Err(exit) =
                        check_stdout_write(writeln!(out, "version current: pin {pin}"))
                    {
                        return exit;
                    }
                }
                return 0;
            }
            if is_json {
                if let Err(exit) = emit_started(invocation, out) {
                    return exit;
                }
            }
            return operational(
                invocation,
                out,
                err,
                CODE_STATUS_PIN_MISMATCH,
                &format!(
                    "version drift: pin {current:?} needs {pin:?} (run `dx version --pin {pin} --apply`)"
                ),
            );
        }
        return match dx_adopt::write_version_pin(workspace, pin) {
            Ok(()) => {
                if is_json {
                    if let Ok(event) = command_started(invocation.command.name(), false, "default")
                    {
                        if let Err(exit) = emit_event(out, &event) {
                            return exit;
                        }
                    }
                    if let Err(exit) = json_status(
                        out,
                        "pin",
                        "ok",
                        pin,
                        &format!("dx version --pin {}", dx_adopt::MODULE_VERSION),
                    ) {
                        return exit;
                    }
                    if let Err(exit) =
                        emit_event(out, &command_finished(0, &FinishedCounts::default()))
                    {
                        return exit;
                    }
                    return 0;
                }
                if let Err(exit) = check_stdout_write(writeln!(out, "pinned {pin}")) {
                    return exit;
                }
                0
            }
            Err(error) => {
                let message = error.to_string();
                if is_json {
                    if let Err(exit) = emit_started(invocation, out) {
                        return exit;
                    }
                }
                operational(invocation, out, err, CODE_STATUS_PIN_MISMATCH, &message)
            }
        };
    }
    if invocation.dry_run {
        if is_json {
            return json_dry_run(invocation, out);
        }
        if !summaries_suppressed(invocation) {
            if invocation.check {
                if let Err(exit) = check_stdout_write(writeln!(out, "would check version pin")) {
                    return exit;
                }
            } else {
                if let Err(exit) = check_stdout_write(writeln!(out, "would report version")) {
                    return exit;
                }
            }
        }
        return 0;
    }
    let current = match dx_adopt::read_version_pin(workspace) {
        Ok(pin) => pin,
        Err(error) => {
            let message = error.to_string();
            if is_json {
                if let Err(exit) = emit_started(invocation, out) {
                    return exit;
                }
            }
            return operational(invocation, out, err, CODE_STATUS_PIN_MISMATCH, &message);
        }
    };
    if invocation.check {
        let detail = format!("dx {current} vs module {}", dx_adopt::MODULE_VERSION);
        let hint = format!("dx version --pin {}", dx_adopt::MODULE_VERSION);
        if dx_adopt::version_pin_matches_module(&current, dx_adopt::MODULE_VERSION) {
            if is_json {
                if let Ok(event) = command_started(invocation.command.name(), false, "default") {
                    if let Err(exit) = emit_event(out, &event) {
                        return exit;
                    }
                }
                if let Err(exit) = json_status(out, "pin", "ok", &detail, &hint) {
                    return exit;
                }
                if let Err(exit) = emit_event(out, &command_finished(0, &FinishedCounts::default()))
                {
                    return exit;
                }
                return 0;
            }
            if let Err(exit) = check_stdout_write(writeln!(out, "version ok: {current}")) {
                return exit;
            }
            0
        } else {
            let message = format!("version drift: {current} != {}", dx_adopt::MODULE_VERSION);
            if is_json {
                if let Err(exit) = emit_started(invocation, out) {
                    return exit;
                }
                if let Err(exit) = json_status(out, "pin", "error", &detail, &hint) {
                    return exit;
                }
            }
            operational(invocation, out, err, CODE_STATUS_PIN_MISMATCH, &message)
        }
    } else {
        if is_json {
            if let Ok(event) = command_started(invocation.command.name(), false, "default") {
                if let Err(exit) = emit_event(out, &event) {
                    return exit;
                }
            }
            if let Err(exit) = json_status(out, "binary", "ok", dx_adopt::DX_VERSION, "dx version")
            {
                return exit;
            }
            if let Err(exit) =
                json_status(out, "module", "ok", dx_adopt::MODULE_VERSION, "dx version")
            {
                return exit;
            }
            if let Err(exit) = json_status(
                out,
                "pin",
                "ok",
                &current,
                &format!("dx version --pin {}", dx_adopt::MODULE_VERSION),
            ) {
                return exit;
            }
            if let Err(exit) = emit_event(out, &command_finished(0, &FinishedCounts::default())) {
                return exit;
            }
            return 0;
        }
        if let Err(exit) = check_stdout_write(writeln!(out, "dx {}", dx_adopt::DX_VERSION)) {
            return exit;
        }
        if let Err(exit) =
            check_stdout_write(writeln!(out, "rules_dx {}", dx_adopt::MODULE_VERSION))
        {
            return exit;
        }
        if let Err(exit) = check_stdout_write(writeln!(out, "pin {current}")) {
            return exit;
        }
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adopt::test_support::{
        event_kinds, events_of_kind, invocation, json_events, run, Truncated,
    };

    #[test]
    fn version_output_boundaries_propagate_broken_pipes() {
        for (words, pin, json_lines, text_lines) in [
            (vec!["version"], Some("0.0.0"), 5, 3),
            (vec!["version", "--check"], Some("0.0.0"), 3, 1),
            (vec!["version", "--check"], Some("9.9.9"), 4, 0),
            (vec!["version", "--apply", "--pin=0.0.0"], None, 3, 1),
            (vec!["version", "--apply", "--pin=9.9.9"], None, 3, 0),
            (
                vec!["version", "--apply", "--rollback"],
                Some("9.9.9"),
                3,
                1,
            ),
            (vec!["version", "--apply", "--rollback"], Some(""), 3, 0),
            (vec!["version", "--apply", "--rollback"], None, 3, 0),
            (vec!["version"], None, 3, 0),
        ] {
            for json in [false, true] {
                for lines in 0..if json { json_lines } else { text_lines } {
                    let scratch = dx_test_scratch::scratch("version-pipe-");
                    if let Some(pin) = pin {
                        std::fs::create_dir(scratch.path().join(".dx")).expect("dx");
                        std::fs::write(scratch.path().join(".dx/version"), pin).expect("seed pin");
                    }
                    let mut inv = invocation(&words);
                    if json {
                        inv.output = OutputMode::Json;
                    }
                    assert_eq!(
                        execute_version(
                            &inv,
                            scratch.path(),
                            &mut Truncated::after_lines(lines),
                            &mut Vec::new()
                        ),
                        141,
                        "{words:?} json={json} lines={lines}"
                    );
                }
            }
        }
        for words in [
            vec!["version"],
            vec!["version", "--check"],
            vec!["version", "--pin=0.0.0"],
            vec!["version", "--rollback"],
        ] {
            for json in [false, true] {
                for lines in 0..if json { 2 } else { 1 } {
                    let scratch = dx_test_scratch::scratch("version-dry-pipe-");
                    dx_adopt::write_version_pin(scratch.path(), "9.9.9").expect("seed pin");
                    let mut inv = invocation(&words);
                    inv.dry_run = true;
                    if json {
                        inv.output = OutputMode::Json;
                    }
                    assert_eq!(
                        execute_version(
                            &inv,
                            scratch.path(),
                            &mut Truncated::after_lines(lines),
                            &mut Vec::new()
                        ),
                        141
                    );
                    assert_eq!(
                        dx_adopt::read_version_pin(scratch.path()).expect("pin"),
                        "9.9.9"
                    );
                }
            }
        }
    }

    #[test]
    fn version_pin_io_failure_is_operational_in_text_and_json() {
        for json in [false, true] {
            let scratch = dx_test_scratch::scratch("version-pin-collision-");
            std::fs::write(scratch.path().join(".dx"), "foreign file").expect("collision");
            let mut inv = invocation(&["version", "--apply", "--pin=0.0.0"]);
            if json {
                inv.output = OutputMode::Json;
            }
            let mut out = Vec::new();
            let mut err = Vec::new();
            assert_eq!(execute_version(&inv, scratch.path(), &mut out, &mut err), 1);
            assert!(!err.is_empty());
            assert_eq!(
                std::fs::read_to_string(scratch.path().join(".dx")).expect("foreign"),
                "foreign file"
            );
            if json {
                let events: Vec<serde_json::Value> = String::from_utf8(out)
                    .expect("stdout")
                    .lines()
                    .map(|line| serde_json::from_str(line).expect("event"))
                    .collect();
                assert_eq!(events[1]["code"], CODE_STATUS_PIN_MISMATCH);
                assert_eq!(events[2]["exit_code"], 1);
            }
        }
    }

    #[test]
    fn version_pins_and_reports() {
        let scratch = dx_test_scratch::scratch("dx-adopt-version-pins-");
        let root = scratch.path().to_path_buf();
        let pin = invocation(&["version", "--apply", "--pin=0.0.0"]);
        let (code, _out, _err) = run(&pin, &root);
        assert_eq!(code, 0);
        assert!(root.join(".dx/version").exists());
    }

    #[test]
    fn version_pin_checks_without_writing_and_apply_writes() {
        let scratch = dx_test_scratch::scratch("dx-adopt-version-pin-check-");
        let root = scratch.path().to_path_buf();
        let inv = invocation(&["version", "--pin=0.0.0"]);
        let (code, _out, err) = run(&inv, &root);
        assert_eq!(code, 1);
        assert!(err.contains("status_pin_mismatch"), "{err}");
        assert!(err.contains("drift"), "{err}");
        assert!(err.contains("--apply"), "{err}");
        assert!(!root.join(".dx/version").exists(), "check must not write");
        let inv = invocation(&["version", "--apply", "--pin=0.0.0"]);
        let (code, _out, _err) = run(&inv, &root);
        assert_eq!(code, 0);
        assert!(root.join(".dx/version").exists());
        let inv = invocation(&["version", "--pin=0.0.0"]);
        let (code, out, _err) = run(&inv, &root);
        assert_eq!(code, 0, "{out}");
        assert!(out.contains("current"), "{out}");
    }

    #[test]
    fn version_rollback_pins_previous_release() {
        let scratch = dx_test_scratch::scratch("dx-adopt-version-rollback-");
        let root = scratch.path().to_path_buf();
        let inv = invocation(&["version", "--rollback"]);
        let (code, _out, err) = run(&inv, &root);
        assert_eq!(code, 1);
        assert!(err.contains("read version pin"));
        assert!(!root.join(".dx/version").exists());
        std::fs::create_dir_all(root.join(".dx")).expect("dx");
        std::fs::write(root.join(".dx/version"), "\n").expect("empty pin");
        let (code, _out, err) = run(&inv, &root);
        assert_eq!(code, 1);
        assert!(err.contains("rollback refused"));
        std::fs::write(root.join(".dx/version"), "9.9.9\n").expect("drifted pin");
        let apply = invocation(&["version", "--apply", "--rollback"]);
        let (code, out, _err) = run(&apply, &root);
        assert_eq!(code, 0);
        assert!(out.contains("rollback"));
        let pinned = std::fs::read_to_string(root.join(".dx/version")).expect("pin");
        assert_eq!(pinned.trim(), dx_adopt::PREVIOUS_VERSION);
        let (code, _out, err) = run(&apply, &root);
        assert_eq!(code, 1);
        assert!(err.contains("rollback refused"));
    }

    #[test]
    fn version_rollback_checks_without_writing() {
        let scratch = dx_test_scratch::scratch("dx-adopt-version-rollback-check-");
        let root = scratch.path().to_path_buf();
        std::fs::create_dir_all(root.join(".dx")).expect("dx");
        std::fs::write(root.join(".dx/version"), "9.9.9\n").expect("drifted pin");
        let inv = invocation(&["version", "--rollback"]);
        let (code, _out, err) = run(&inv, &root);
        assert_eq!(code, 1);
        assert!(err.contains("drift"), "{err}");
        assert!(err.contains("--apply"), "{err}");
        assert_eq!(
            std::fs::read_to_string(root.join(".dx/version")).expect("pin"),
            "9.9.9\n",
            "check must not write"
        );
    }

    #[test]
    fn version_rejects_combined_mutation_and_check_flags() {
        let scratch = dx_test_scratch::scratch("dx-adopt-version-conflicts-");
        let root = scratch.path().to_path_buf();
        for words in [
            vec!["version", "--pin=0.0.0", "--rollback"],
            vec!["version", "--pin=0.0.0", "--check"],
            vec!["version", "--rollback", "--check"],
        ] {
            let inv = invocation(&words);
            let (code, _out, _err) = run(&inv, &root);
            assert_eq!(code, 2, "words: {words:?}");
        }
    }

    #[test]
    fn version_check_reports_drift() {
        let scratch = dx_test_scratch::scratch("dx-adopt-version-check-");
        let root = scratch.path().to_path_buf();
        std::fs::create_dir_all(root.join(".dx")).expect("dx");
        std::fs::write(root.join(".dx/version"), "0.0.0\n").expect("pin");
        let inv = invocation(&["version", "--check"]);
        let (code, out, _err) = run(&inv, &root);
        assert_eq!(code, 0);
        assert!(out.contains("version ok"));
        std::fs::write(root.join(".dx/version"), "0.1.0\n").expect("drift");
        let (code, _out, err) = run(&inv, &root);
        assert_eq!(code, 1);
        assert!(err.contains("drift"));
    }

    #[test]
    fn version_missing_pin_fails_closed() {
        for words in [vec!["version", "--check"], vec!["version"]] {
            let scratch = dx_test_scratch::scratch("dx-adopt-version-missing-");
            let root = scratch.path().to_path_buf();
            let inv = invocation(&words);
            let (code, out, err) = run(&inv, &root);
            assert_eq!(code, 1, "words: {words:?}");
            assert!(err.contains("read version pin"), "words: {words:?}");
            assert!(!out.contains("version ok"), "words: {words:?}");
        }
    }

    #[test]
    fn version_bare_and_check_dry_run_plan_without_reading() {
        let scratch = dx_test_scratch::scratch("dx-adopt-version-dry-bare-");
        let root = scratch.path().to_path_buf();
        for (words, want) in [
            (vec!["version", "--dry-run"], "would report version"),
            (
                vec!["version", "--check", "--dry-run"],
                "would check version pin",
            ),
        ] {
            let inv = invocation(&words);
            let (code, out, _err) = run(&inv, &root);
            assert_eq!(code, 0, "words: {words:?}");
            assert!(out.contains(want), "words: {words:?}");
        }
    }

    #[test]
    fn version_json_streams_status_envelope() {
        let scratch = dx_test_scratch::scratch("dx-adopt-version-json-");
        let root = scratch.path().to_path_buf();
        std::fs::create_dir_all(root.join(".dx")).expect("dx");
        std::fs::write(root.join(".dx/version"), "0.0.0\n").expect("pin");
        let inv = invocation(&["version", "--output=json"]);
        let (code, out, err) = run(&inv, &root);
        assert_eq!(code, 0);
        let text = out;
        let events = json_events(&text);
        let kinds = event_kinds(&events);
        assert_eq!(kinds[0], "command_started");
        assert_eq!(kinds[kinds.len() - 1], "command_finished");
        assert!(kinds.contains(&"status"), "{kinds:?}");
        assert!(!kinds.contains(&"error"), "{kinds:?}");
        assert_eq!(
            events.last().expect("finished")["exit_code"],
            serde_json::json!(0)
        );
        let names: Vec<&str> = events_of_kind(&events, "status")
            .into_iter()
            .map(|event| event["name"].as_str().expect("name"))
            .collect();
        assert_eq!(names, vec!["binary", "module", "pin"], "{names:?}");
        assert!(err.is_empty());
    }

    #[test]
    fn version_check_json_reports_drift_with_error() {
        let scratch = dx_test_scratch::scratch("dx-adopt-version-check-json-");
        let root = scratch.path().to_path_buf();
        std::fs::create_dir_all(root.join(".dx")).expect("dx");
        std::fs::write(root.join(".dx/version"), "0.0.0\n").expect("pin");
        let inv = invocation(&["version", "--check", "--output=json"]);
        let (code, out, _err) = run(&inv, &root);
        assert_eq!(code, 0);
        let text = out;
        let events = json_events(&text);
        let kinds = event_kinds(&events);
        assert_eq!(
            kinds,
            vec!["command_started", "status", "command_finished"],
            "{kinds:?}"
        );
        std::fs::write(root.join(".dx/version"), "9.9.9\n").expect("drift");
        let (code, out, err) = run(&inv, &root);
        assert_eq!(code, 1);
        let text = out;
        let events = json_events(&text);
        let kinds = event_kinds(&events);
        assert_eq!(
            kinds,
            vec!["command_started", "status", "error", "command_finished"],
            "{kinds:?}"
        );
        assert_eq!(
            events.last().expect("finished")["exit_code"],
            serde_json::json!(1)
        );
        assert_eq!(
            events[2]["code"],
            serde_json::json!(CODE_STATUS_PIN_MISMATCH)
        );
        assert!(err.contains("drift"));
    }

    #[test]
    fn version_dry_run_json_emits_lifecycle_only() {
        let scratch = dx_test_scratch::scratch("dx-adopt-version-dry-json-");
        let root = scratch.path().to_path_buf();
        for words in [
            vec!["version", "--dry-run", "--output=json"],
            vec!["version", "--check", "--dry-run", "--output=json"],
        ] {
            let inv = invocation(&words);
            let (code, out, _err) = run(&inv, &root);
            assert_eq!(code, 0, "words: {words:?}");
            let text = out;
            let events = json_events(&text);
            let kinds = event_kinds(&events);
            assert_eq!(
                kinds,
                vec!["command_started", "command_finished"],
                "{words:?}"
            );
            assert_eq!(events[0]["dry_run"], serde_json::json!(true), "{words:?}");
        }
    }

    #[test]
    fn version_missing_pin_json_fails_closed() {
        let scratch = dx_test_scratch::scratch("dx-adopt-version-missing-json-");
        let root = scratch.path().to_path_buf();
        let inv = invocation(&["version", "--output=json"]);
        let (code, out, _err) = run(&inv, &root);
        assert_eq!(code, 1);
        let text = out;
        let events = json_events(&text);
        let kinds = event_kinds(&events);
        assert_eq!(
            kinds,
            vec!["command_started", "error", "command_finished"],
            "{kinds:?}"
        );
        assert_eq!(
            events[1]["code"],
            serde_json::json!(CODE_STATUS_PIN_MISMATCH)
        );
    }
}
