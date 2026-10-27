use std::io::Write;

use crate::args::Invocation;
use crate::exec::common::{check_stdout_write, emit_built, emit_event, emit_started};
use dx_output::{command_finished, command_started, status_event, FinishedCounts, StatusEvent};

use super::status::CODE_STATUS_PIN_MISMATCH;
use super::{operational, summaries_suppressed};

fn json_pin(
    invocation: &Invocation,
    out: &mut dyn Write,
    mode: &str,
    detail: &str,
) -> Result<(), i32> {
    emit_built(out, command_started(invocation.command.name(), mode))?;
    emit_built(
        out,
        status_event(&StatusEvent {
            name: "pin".to_owned(),
            status: "ok".to_owned(),
            detail: detail.to_owned(),
            hint: "dx rollback".to_owned(),
        }),
    )?;
    emit_event(out, &command_finished(0, &FinishedCounts::default()))
}

fn refuse(invocation: &Invocation, out: &mut dyn Write, err: &mut dyn Write, message: &str) -> i32 {
    if invocation.output == dx_output::OutputMode::Json {
        if let Err(exit) = emit_started(invocation, out) {
            return exit;
        }
    }
    operational(invocation, out, err, CODE_STATUS_PIN_MISMATCH, message)
}

pub(crate) fn execute_rollback(
    invocation: &Invocation,
    workspace: &std::path::Path,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let is_json = invocation.output == dx_output::OutputMode::Json;
    let record = match dx_adopt::read_pin_record(workspace) {
        Ok(record) => record,
        Err(error) => return refuse(invocation, out, err, &error.to_string()),
    };
    let current: Option<String> = match dx_adopt::read_version_pin(workspace) {
        Ok(pin) if pin.is_empty() => None,
        Ok(pin) => Some(pin),
        Err(_) if !workspace.join(".dx/version").is_file() => None,
        Err(error) => return refuse(invocation, out, err, &error.to_string()),
    };
    let recorded = record.current.as_deref().filter(|pin| !pin.is_empty());
    if current.as_deref() != recorded {
        let message = format!(
            "rollback refused: pin {:?} does not match recorded {:?} (the pin changed outside dx; nothing was written)",
            current.as_deref().unwrap_or("no pin"),
            recorded.unwrap_or("no pin"),
        );
        return refuse(invocation, out, err, &message);
    }
    let target = record
        .previous
        .clone()
        .filter(|previous| !previous.is_empty());
    if current.is_none() && target.is_none() {
        return refuse(
            invocation,
            out,
            err,
            "rollback refused: no earlier pin recorded",
        );
    }
    if !invocation.applies() {
        if target == current {
            let at = target.as_deref().unwrap_or_default();
            if is_json {
                return match json_pin(invocation, out, "check", at) {
                    Ok(()) => 0,
                    Err(exit) => exit,
                };
            }
            if !summaries_suppressed(invocation) {
                if let Err(exit) = check_stdout_write(writeln!(out, "pin current: rollback {at}")) {
                    return exit;
                }
            }
            return 0;
        }
        let at = current.as_deref().unwrap_or("no pin");
        let message = match &target {
            Some(previous) => {
                format!("pin {at:?} can roll back to {previous:?} (run `dx rollback --apply`)")
            }
            None => {
                format!("pin {at:?} has no earlier pin (run `dx rollback --apply` to remove it)")
            }
        };
        return refuse(invocation, out, err, &message);
    }
    if let Some(previous) = &target {
        if let Err(error) = dx_adopt::plan_launcher_files(workspace, previous, false) {
            let message = format!("rollback refused: {error} (nothing was written)");
            return refuse(invocation, out, err, &message);
        }
    }
    let landed = match &target {
        Some(previous) => {
            dx_adopt::write_version_pin(workspace, previous).map_err(|e| e.to_string())
        }
        None => std::fs::remove_file(workspace.join(".dx/version")).map_err(|e| e.to_string()),
    };
    if let Err(detail) = landed {
        return refuse(invocation, out, err, &detail);
    }
    if let Err(error) =
        dx_adopt::record_pin_operation(workspace, current.as_deref(), target.as_deref())
    {
        let at = current.as_deref().unwrap_or("no pin");
        let message = match &target {
            Some(previous) => format!("pinned {previous} but history unavailable: {error}"),
            None => format!("removed pin {at} but history unavailable: {error}"),
        };
        return refuse(invocation, out, err, &message);
    }
    let refreshed = match &target {
        Some(previous) => match dx_adopt::refresh_launcher_files(workspace, previous) {
            Ok(wrote) => wrote,
            Err(error) => {
                let message = format!(
                    "pinned {previous} but launchers not refreshed: {error} (run `bazel run @rules_dx//:dx -- init --apply`)"
                );
                return refuse(invocation, out, err, &message);
            }
        },
        None => Vec::new(),
    };
    if is_json {
        return match json_pin(invocation, out, "default", target.as_deref().unwrap_or("")) {
            Ok(()) => 0,
            Err(exit) => exit,
        };
    }
    let line = match &target {
        Some(previous) => format!("pinned {previous} (rollback)"),
        None => format!(
            "removed pin {} (rollback)",
            current.as_deref().unwrap_or("no pin")
        ),
    };
    if let Err(exit) = check_stdout_write(writeln!(out, "{line}")) {
        return exit;
    }
    if !summaries_suppressed(invocation) {
        for rel in refreshed {
            if let Err(exit) = check_stdout_write(writeln!(out, "wrote {rel}")) {
                return exit;
            }
        }
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adopt::test_support::{event_kinds, invocation, json_events, run, Truncated};
    use dx_output::OutputMode;

    fn read_pin(root: &std::path::Path) -> String {
        std::fs::read_to_string(root.join(".dx/version")).expect("pin")
    }

    #[test]
    fn rollback_output_boundaries_propagate_broken_pipes() {
        for (words, json_lines, text_lines) in [
            (vec!["rollback", "--apply"], 3, 1),
            (vec!["rollback"], 3, 0),
        ] {
            for json in [false, true] {
                for lines in 0..if json { json_lines } else { text_lines } {
                    let scratch = dx_test_scratch::scratch("rollback-pipe-");
                    dx_adopt::write_version_pin(scratch.path(), "9.9.9").expect("seed pin");
                    dx_adopt::record_pin_operation(scratch.path(), Some("0.0.0"), Some("9.9.9"))
                        .expect("seed record");
                    let mut inv = invocation(&words);
                    if json {
                        inv.output = OutputMode::Json;
                    }
                    let code = execute_rollback(
                        &inv,
                        scratch.path(),
                        &mut Truncated::after_lines(lines),
                        &mut Vec::new(),
                    );
                    assert!(
                        code == 141 || code == 1,
                        "{words:?} json={json} lines={lines}: {code}"
                    );
                }
            }
        }
    }

    #[test]
    fn rollback_without_a_record_refuses_and_writes_nothing() {
        let scratch = dx_test_scratch::scratch("dx-adopt-rollback-none-");
        let root = scratch.path().to_path_buf();
        for words in [vec!["rollback"], vec!["rollback", "--apply"]] {
            let (code, _out, err) = run(&invocation(&words), &root);
            assert_eq!(code, 1, "{words:?}");
            assert!(err.contains("no recorded pin operation"), "{err}");
            assert!(!root.join(".dx").exists(), "{words:?} wrote state");
        }
    }

    #[test]
    fn rollback_refuses_after_a_manual_pin_edit() {
        let scratch = dx_test_scratch::scratch("dx-adopt-rollback-edited-");
        let root = scratch.path().to_path_buf();
        dx_adopt::write_version_pin(&root, "0.0.0").expect("pin");
        dx_adopt::record_pin_operation(&root, None, Some("0.0.0")).expect("record");
        std::fs::write(root.join(".dx/version"), "9.9.9\n").expect("manual edit");
        let (code, _out, err) = run(&invocation(&["rollback", "--apply"]), &root);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("does not match recorded"), "{err}");
        assert_eq!(read_pin(&root), "9.9.9\n");
    }

    #[test]
    fn rollback_checks_without_writing_and_apply_restores_the_recorded_pin() {
        let scratch = dx_test_scratch::scratch("dx-adopt-rollback-apply-");
        let root = scratch.path().to_path_buf();
        dx_adopt::write_version_pin(&root, "9.9.9").expect("pin");
        dx_adopt::record_pin_operation(&root, Some("0.0.0"), Some("9.9.9")).expect("record");
        let (code, _out, err) = run(&invocation(&["rollback"]), &root);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("status_pin_mismatch"), "{err}");
        assert!(err.contains("dx rollback --apply"), "{err}");
        assert_eq!(read_pin(&root), "9.9.9\n", "check must not write");
        let apply = invocation(&["rollback", "--apply"]);
        let (code, out, _err) = run(&apply, &root);
        assert_eq!(code, 0, "{out}");
        assert!(out.contains("pinned 0.0.0 (rollback)"), "{out}");
        assert_eq!(read_pin(&root), "0.0.0\n");
        let (code, out, _err) = run(&invocation(&["rollback"]), &root);
        assert_eq!(code, 1, "{out}");
        let (code, _out, _err) = run(&apply, &root);
        assert_eq!(code, 0);
        assert_eq!(
            read_pin(&root),
            "9.9.9\n",
            "rollback chains back through the new record"
        );
    }

    #[test]
    fn rollback_apply_refreshes_a_stale_launcher() {
        let scratch = dx_test_scratch::scratch("dx-adopt-rollback-launcher-");
        let root = scratch.path().to_path_buf();
        dx_adopt::write_version_pin(&root, "9.9.9").expect("pin");
        dx_adopt::record_pin_operation(&root, Some("0.0.1"), Some("9.9.9")).expect("record");
        dx_adopt::write_launcher_files(&root, "9.9.9", &mut dx_adopt::Applied::default())
            .expect("launchers");
        std::fs::write(root.join("tools/dx.cmd"), "@echo off\r\nmine\r\n").expect("custom");
        let (code, out, err) = run(&invocation(&["rollback", "--apply"]), &root);
        assert_eq!(code, 0, "{err}");
        assert_eq!(out, "pinned 0.0.1 (rollback)\nwrote tools/dx\n");
        assert_eq!(read_pin(&root), "0.0.1\n");
        assert_eq!(
            dx_adopt::launcher_version(&root, "tools/dx").as_deref(),
            Some("0.0.1")
        );
        assert_eq!(
            std::fs::read_to_string(root.join("tools/dx.cmd")).expect("cmd"),
            "@echo off\r\nmine\r\n"
        );
    }

    #[test]
    fn rollback_apply_creates_no_launcher() {
        let scratch = dx_test_scratch::scratch("dx-adopt-rollback-no-launcher-");
        let root = scratch.path().to_path_buf();
        dx_adopt::write_version_pin(&root, "9.9.9").expect("pin");
        dx_adopt::record_pin_operation(&root, Some("0.0.1"), Some("9.9.9")).expect("record");
        let (code, out, err) = run(&invocation(&["rollback", "--apply"]), &root);
        assert_eq!(code, 0, "{err}");
        assert_eq!(out, "pinned 0.0.1 (rollback)\n");
        assert!(!root.join("tools").exists());
    }

    #[test]
    fn rollback_to_a_non_semver_pin_with_a_launcher_writes_nothing() {
        let scratch = dx_test_scratch::scratch("dx-adopt-rollback-launcher-bad-");
        let root = scratch.path().to_path_buf();
        dx_adopt::write_version_pin(&root, "9.9.9").expect("pin");
        dx_adopt::record_pin_operation(&root, Some("not-semver"), Some("9.9.9")).expect("record");
        let before = std::fs::read(root.join(dx_adopt::VERSION_HISTORY_REL)).expect("history");
        dx_adopt::write_launcher_files(&root, "9.9.9", &mut dx_adopt::Applied::default())
            .expect("launchers");
        let (code, _out, err) = run(&invocation(&["rollback", "--apply"]), &root);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("rollback refused: launcher version"), "{err}");
        assert!(err.contains("nothing was written"), "{err}");
        assert_eq!(read_pin(&root), "9.9.9\n");
        assert_eq!(
            std::fs::read(root.join(dx_adopt::VERSION_HISTORY_REL)).expect("history"),
            before
        );
        assert_eq!(
            dx_adopt::launcher_version(&root, "tools/dx").as_deref(),
            Some("9.9.9")
        );
    }

    #[cfg(unix)]
    #[test]
    fn rollback_refresh_failure_keeps_pin_and_history_in_step() {
        use std::os::unix::fs::PermissionsExt;
        let scratch = dx_test_scratch::scratch("dx-adopt-rollback-launcher-io-");
        let root = scratch.path().to_path_buf();
        dx_adopt::write_version_pin(&root, "9.9.9").expect("pin");
        dx_adopt::record_pin_operation(&root, Some("0.0.1"), Some("9.9.9")).expect("record");
        dx_adopt::write_launcher_files(&root, "9.9.9", &mut dx_adopt::Applied::default())
            .expect("launchers");
        let tools = root.join("tools");
        std::fs::set_permissions(&tools, std::fs::Permissions::from_mode(0o555)).expect("lock");
        let (code, _out, err) = run(&invocation(&["rollback", "--apply"]), &root);
        std::fs::set_permissions(&tools, std::fs::Permissions::from_mode(0o755)).expect("unlock");
        assert_eq!(code, 1, "{err}");
        assert!(
            err.contains("pinned 0.0.1 but launchers not refreshed"),
            "{err}"
        );
        assert!(err.contains("init --apply"), "{err}");
        assert_eq!(read_pin(&root), "0.0.1\n");
        let record = dx_adopt::read_pin_record(&root).expect("record");
        assert_eq!(record.current.as_deref(), Some("0.0.1"));
    }

    #[test]
    fn rollback_without_a_previous_pin_removes_the_pin() {
        let scratch = dx_test_scratch::scratch("dx-adopt-rollback-bare-");
        let root = scratch.path().to_path_buf();
        dx_adopt::write_version_pin(&root, "0.0.0").expect("pin");
        dx_adopt::record_pin_operation(&root, None, Some("0.0.0")).expect("record");
        let (code, out, _err) = run(&invocation(&["rollback", "--apply"]), &root);
        assert_eq!(code, 0, "{out}");
        assert!(out.contains("removed pin"), "{out}");
        assert!(!root.join(".dx/version").exists());
    }

    #[test]
    fn rollback_restores_a_removed_pin_from_the_record() {
        let scratch = dx_test_scratch::scratch("dx-adopt-rollback-restore-");
        let root = scratch.path().to_path_buf();
        dx_adopt::record_pin_operation(&root, Some("0.0.0"), None).expect("record");
        let (code, _out, err) = run(&invocation(&["rollback"]), &root);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("0.0.0"), "{err}");
        assert!(!root.join(".dx/version").exists(), "check must not write");
        let (code, out, _err) = run(&invocation(&["rollback", "--apply"]), &root);
        assert_eq!(code, 0, "{out}");
        assert!(out.contains("pinned 0.0.0 (rollback)"), "{out}");
        assert_eq!(read_pin(&root), "0.0.0\n");
        let record = dx_adopt::read_pin_record(&root).expect("record");
        assert_eq!(record.previous, None);
        assert_eq!(record.current.as_deref(), Some("0.0.0"));
    }

    #[test]
    fn rollback_io_failure_is_operational_in_text_and_json() {
        for json in [false, true] {
            let scratch = dx_test_scratch::scratch("rollback-collision-");
            std::fs::write(scratch.path().join(".dx"), "foreign file").expect("collision");
            let mut inv = invocation(&["rollback", "--apply"]);
            if json {
                inv.output = OutputMode::Json;
            }
            let mut out = Vec::new();
            let mut err = Vec::new();
            assert_eq!(
                execute_rollback(&inv, scratch.path(), &mut out, &mut err),
                1
            );
            assert!(!err.is_empty());
            assert_eq!(
                std::fs::read_to_string(scratch.path().join(".dx")).expect("foreign"),
                "foreign file"
            );
        }
    }

    #[test]
    fn rollback_json_streams_the_status_envelope() {
        let scratch = dx_test_scratch::scratch("dx-adopt-rollback-json-");
        let root = scratch.path().to_path_buf();
        dx_adopt::write_version_pin(&root, "9.9.9").expect("pin");
        dx_adopt::record_pin_operation(&root, Some("0.0.0"), Some("9.9.9")).expect("record");
        let (code, out, _err) = run(
            &invocation(&["rollback", "--apply", "--output=json"]),
            &root,
        );
        assert_eq!(code, 0, "{out}");
        let events = json_events(&out);
        assert_eq!(
            event_kinds(&events),
            vec!["command_started", "status", "command_finished"]
        );
        let (code, out, _err) = run(&invocation(&["rollback", "--output=json"]), &root);
        assert_eq!(code, 1);
        let events = json_events(&out);
        assert_eq!(
            event_kinds(&events),
            vec!["command_started", "error", "command_finished"]
        );
        assert_eq!(events[1]["code"], CODE_STATUS_PIN_MISMATCH);
    }
}
