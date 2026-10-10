use std::io::Write;

use crate::args::Invocation;
use crate::exec::common::{check_stdout_write, emit_event};

use dx_output::{
    command_finished, command_started, notice_event, FinishedCounts, NoticeEvent, OutputMode,
};

use super::{operational, pre_exec, summaries_suppressed};

pub(crate) const CODE_UPGRADE_FAILED: &str = "upgrade_failed";

pub(crate) const NOTICE_UPGRADE_PLANNED: &str = "upgrade_planned";

pub(crate) fn execute_upgrade(
    invocation: &Invocation,
    _workspace: &std::path::Path,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let (Some(from), Some(to)) = (invocation.from.as_deref(), invocation.to.as_deref()) else {
        return pre_exec(err, "upgrade needs --from <version> --to <version>");
    };
    let plan = match dx_adopt::plan_upgrade(from, to) {
        Ok(plan) => plan,
        Err(error) => return pre_exec(err, &error.to_string()),
    };
    if let Some(incompatible) = dx_adopt::module_incompatible_with_binary(_workspace) {
        let mode = if invocation.applies() {
            "default"
        } else {
            "check"
        };
        if invocation.output == OutputMode::Json {
            if let Ok(event) = command_started(invocation.command.name(), false, mode) {
                if let Err(exit) = emit_event(out, &event) {
                    return exit;
                }
            }
        }
        return operational(invocation, out, err, CODE_UPGRADE_FAILED, &incompatible);
    }
    if !dx_adopt::upgrade_manifest_available(from, to) {
        let reason = dx_adopt::upgrade_unavailable_reason(from, to, &plan.manifest);
        let message = format!("{reason}; {}", plan.message);
        if invocation.dry_run {
            if invocation.output == OutputMode::Json {
                if let Ok(event) = command_started(invocation.command.name(), true, "default") {
                    if let Err(exit) = emit_event(out, &event) {
                        return exit;
                    }
                }
                return operational(invocation, out, err, CODE_UPGRADE_FAILED, &message);
            }
            return operational(invocation, out, err, CODE_UPGRADE_FAILED, &message);
        }
        let verbose = invocation.chatty();
        let mode = if invocation.applies() {
            "default"
        } else {
            "check"
        };
        if invocation.output == OutputMode::Json {
            if let Ok(event) = command_started(invocation.command.name(), false, mode) {
                if let Err(exit) = emit_event(out, &event) {
                    return exit;
                }
            }
            return operational(invocation, out, err, CODE_UPGRADE_FAILED, &message);
        }
        if verbose {
            let live_summary = format!(
                "Upgrade {} -> {} via {} (pin {}, migrate, setup)",
                plan.from, plan.to, plan.manifest, plan.to
            );
            if let Err(exit) = check_stdout_write(writeln!(out, "{live_summary}")) {
                return exit;
            }
        }
        return operational(invocation, out, err, CODE_UPGRADE_FAILED, &message);
    }
    let summary = format!(
        "Would upgrade {} -> {} via {} (pin {}, migrate, setup)",
        plan.from, plan.to, plan.manifest, plan.to
    );
    let verbose = invocation.chatty();
    let mode = if invocation.applies() {
        "default"
    } else {
        "check"
    };
    if invocation.dry_run {
        if invocation.output == OutputMode::Json {
            if let Ok(event) = command_started(invocation.command.name(), true, "default") {
                if let Err(exit) = emit_event(out, &event) {
                    return exit;
                }
            }
            if let Ok(event) = notice_event(&NoticeEvent {
                level: "info".to_owned(),
                code: NOTICE_UPGRADE_PLANNED.to_owned(),
                message: summary,
                related_command: Some("upgrade".to_owned()),
                scope: Some(vec![plan.manifest.clone()]),
                path: Some(plan.manifest.clone()),
                language: None,
                import: None,
            }) {
                if let Err(exit) = emit_event(out, &event) {
                    return exit;
                }
            }
            let finished = command_finished(0, &FinishedCounts::default());
            if let Err(exit) = emit_event(out, &finished) {
                return exit;
            }
        } else if !summaries_suppressed(invocation) {
            if let Err(exit) = check_stdout_write(writeln!(out, "{summary}")) {
                return exit;
            }
        }
        return 0;
    }
    let live_summary = format!(
        "Upgrade {} -> {} via {} (pin {}, migrate, setup)",
        plan.from, plan.to, plan.manifest, plan.to
    );
    let message = format!(
        "no upgrade manifest {} yet (module at 0.0.0, no releases cut); {}",
        plan.manifest, plan.message
    );
    if invocation.output == OutputMode::Json {
        if let Ok(event) = command_started(invocation.command.name(), false, mode) {
            if let Err(exit) = emit_event(out, &event) {
                return exit;
            }
        }
        return operational(invocation, out, err, CODE_UPGRADE_FAILED, &message);
    }
    if verbose {
        if let Err(exit) = check_stdout_write(writeln!(out, "{live_summary}")) {
            return exit;
        }
    }
    operational(invocation, out, err, CODE_UPGRADE_FAILED, &message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adopt::test_support::{event, event_kinds, invocation, json_events, run};
    use dx_process::pre_exec_code;

    #[test]
    fn upgrade_dry_run_validates_the_manifest_before_planning() {
        let inv = invocation(&["upgrade", "--from=1.2.3", "--to=2.0.0", "--dry-run"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-upgrade-dry-");
        let root = scratch.path().to_path_buf();
        let (code, _out, err) = run(&inv, &root);
        assert_eq!(code, 1);
        assert!(err.contains(CODE_UPGRADE_FAILED), "{err}");
        assert!(err.contains("migrate-v1-to-v2.json"), "{err}");
        assert!(err.contains("major"), "{err}");
        assert!(err.contains("dx upgrade --from 1.2.3 --to 2.0.0"), "{err}");
        let minor = invocation(&["upgrade", "--from=1.2.3", "--to=1.3.0", "--dry-run"]);
        let (code, _out, err) = run(&minor, &root);
        assert_eq!(code, 1);
        assert!(err.contains("migrate-v1.2.3-to-v1.3.0.json"), "{err}");
    }

    #[test]
    fn upgrade_dry_run_json_fails_closed_with_error_and_finished() {
        let inv = invocation(&[
            "upgrade",
            "--from=1.2.3",
            "--to=2.0.0",
            "--dry-run",
            "--output=json",
        ]);
        let scratch = dx_test_scratch::scratch("dx-adopt-upgrade-dry-json-");
        let root = scratch.path().to_path_buf();
        let (code, out, err) = run(&inv, &root);
        assert_eq!(code, 1);
        assert!(err.contains(CODE_UPGRADE_FAILED), "{err}");
        let text = out;
        let events = json_events(&text);
        let kinds = event_kinds(&events);
        assert_eq!(kinds[0], "command_started");
        assert_eq!(kinds[kinds.len() - 1], "command_finished");
        assert!(kinds.contains(&"error"), "{kinds:?}");
        assert!(!kinds.contains(&"notice"), "{kinds:?}");
        let error = event(&events, "error");
        assert_eq!(error["code"], serde_json::json!(CODE_UPGRADE_FAILED));
        assert_eq!(
            events.last().expect("finished")["exit_code"],
            serde_json::json!(1)
        );
    }

    #[test]
    fn upgrade_refuses_incompatible_modules_before_manifest_checks() {
        let scratch = dx_test_scratch::scratch("dx-adopt-upgrade-skew-");
        let root = scratch.path().to_path_buf();
        std::fs::write(
            root.join("MODULE.bazel"),
            "bazel_dep(name = \"rules_dx\", version = \"9.9.9\")\n",
        )
        .expect("module");
        let inv = invocation(&["upgrade", "--from=9.9.9", "--to=9.9.10", "--dry-run"]);
        let (code, _out, err) = run(&inv, &root);
        assert_eq!(code, 1);
        assert!(err.contains(CODE_UPGRADE_FAILED), "{err}");
        assert!(err.contains("incompatible"), "{err}");
    }

    #[test]
    fn upgrade_live_fails_closed_with_recovery_pointer() {
        let inv = invocation(&["upgrade", "--from=1.2.3", "--to=2.0.0"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-upgrade-live-");
        let root = scratch.path().to_path_buf();
        let (code, _out, err) = run(&inv, &root);
        assert_eq!(code, 1);
        let err_text = err;
        assert!(err_text.contains(CODE_UPGRADE_FAILED), "{err_text}");
        assert!(err_text.contains("migrate-v1-to-v2.json"), "{err_text}");
        assert!(
            err_text.contains("dx upgrade --from 1.2.3 --to 2.0.0"),
            "{err_text}"
        );
        assert!(err_text.contains("dx setup"), "{err_text}");
    }

    #[test]
    fn upgrade_apply_fails_closed_like_default() {
        let inv = invocation(&["upgrade", "--apply", "--from=1.2.3", "--to=2.0.0"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-upgrade-apply-");
        let root = scratch.path().to_path_buf();
        let (code, _out, err) = run(&inv, &root);
        assert_eq!(code, 1);
        assert!(err.contains(CODE_UPGRADE_FAILED), "{err}");
    }

    #[test]
    fn upgrade_live_json_fails_closed_with_error_and_finished() {
        let inv = invocation(&["upgrade", "--from=1.2.3", "--to=2.0.0", "--output=json"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-upgrade-live-json-");
        let root = scratch.path().to_path_buf();
        let (code, out, err) = run(&inv, &root);
        assert_eq!(code, 1);
        assert!(err.contains(CODE_UPGRADE_FAILED));
        let text = out;
        let events = json_events(&text);
        let kinds = event_kinds(&events);
        assert_eq!(kinds[0], "command_started");
        assert!(kinds.contains(&"error"), "{kinds:?}");
        assert_eq!(kinds[kinds.len() - 1], "command_finished");
        let error = event(&events, "error");
        assert_eq!(error["code"], serde_json::json!(CODE_UPGRADE_FAILED));
    }

    #[test]
    fn upgrade_codes_are_stable_single_source() {
        assert_eq!(CODE_UPGRADE_FAILED, "upgrade_failed");
        assert_eq!(NOTICE_UPGRADE_PLANNED, "upgrade_planned");
    }

    #[test]
    fn upgrade_rejects_downgrade_pre_exec() {
        let inv = invocation(&["upgrade", "--from=2.0.0", "--to=1.0.0", "--dry-run"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-upgrade-downgrade-");
        let root = scratch.path().to_path_buf();
        let (code, _out, err) = run(&inv, &root);
        assert_eq!(code, pre_exec_code());
        assert!(err.contains("upgrade-only"));
    }
}
