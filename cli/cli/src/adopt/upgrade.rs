use std::io::Write;

use crate::exec::common::{check_stdout_write, emit_built};
use dx_cli_core::args::Invocation;

use dx_output::{command_started, OutputMode};

use super::{operational, pre_exec};

pub(crate) const CODE_UPGRADE_FAILED: &str = "upgrade_failed";

pub(crate) fn execute_upgrade(
    invocation: &Invocation,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let (Some(from), Some(to)) = (invocation.from.as_deref(), invocation.to.as_deref()) else {
        return pre_exec(
            invocation.output,
            out,
            err,
            "upgrade needs --from <version> --to <version>",
        );
    };
    let intent = dx_adopt::UpgradeIntent {
        apply: invocation.applies(),
        workspace: invocation.workspace.clone(),
    };
    let plan = match dx_adopt::plan_upgrade(from, to, &intent) {
        Ok(plan) => plan,
        Err(error) => return pre_exec(invocation.output, out, err, &error.to_string()),
    };
    let verbose = invocation.chatty();
    let mode = if invocation.applies() {
        "default"
    } else {
        "check"
    };
    let unshipped = dx_adopt::AdoptError::NoQualifiedManifest {
        from: plan.from.clone(),
        to: plan.to.clone(),
        manifest: plan.manifest.clone(),
    };
    let message = format!("{unshipped}; {}", plan.message);
    if invocation.output == OutputMode::Json {
        if let Err(exit) = emit_built(out, command_started(invocation.command.name(), mode)) {
            return exit;
        }
    } else if verbose {
        let live_summary = format!(
            "Upgrade {} -> {} via {} (pin {}, migrate, env, codegen)",
            plan.from, plan.to, plan.manifest, plan.to
        );
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
    fn upgrade_fails_closed_without_a_shipped_manifest() {
        let inv = invocation(&["upgrade", "--from=1.2.3", "--to=2.0.0"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-upgrade-dry-");
        let root = scratch.path().to_path_buf();
        let (code, out, err) = run(&inv, &root);
        assert_eq!(code, 1, "{out}{err}");
        assert!(err.contains(CODE_UPGRADE_FAILED), "{err}");
        assert!(err.contains("migrate-v1-to-v2.json"), "{err}");
        assert!(err.contains("not shipped"), "{err}");
        assert!(err.contains("dx upgrade --from 1.2.3 --to 2.0.0"), "{err}");
        assert!(!out.contains("Would upgrade"), "{out}");
    }

    #[test]
    fn upgrade_unshipped_pair_reports_the_qualification_gap() {
        let inv = invocation(&["upgrade", "-v", "--from=1.2.3", "--to=2.0.0"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-upgrade-unshipped-");
        let root = scratch.path().to_path_buf();
        let (code, out, err) = run(&inv, &root);
        assert_eq!(code, 1, "{out}{err}");
        assert!(
            err.contains(
                "no qualified migration 1.2.3 -> 2.0.0: manifest migrate-v1-to-v2.json is not shipped; recovery: rerun"
            ),
            "{err}"
        );
        assert!(!err.contains("no live"), "{err}");
        assert!(
            out.contains(
                "Upgrade 1.2.3 -> 2.0.0 via migrate-v1-to-v2.json (pin 2.0.0, migrate, env, codegen)"
            ),
            "{out}"
        );
    }

    #[test]
    fn upgrade_json_fails_closed_with_error_and_finished() {
        let inv = invocation(&["upgrade", "--from=1.2.3", "--to=2.0.0", "--output=json"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-upgrade-dry-json-");
        let root = scratch.path().to_path_buf();
        let (code, out, err) = run(&inv, &root);
        assert_eq!(code, 1, "{out}{err}");
        let text = out;
        let events = json_events(&text);
        let kinds = event_kinds(&events);
        assert_eq!(kinds[0], "command_started");
        assert!(kinds.contains(&"error"), "{kinds:?}");
        assert!(!kinds.contains(&"notice"), "{kinds:?}");
        assert_eq!(kinds[kinds.len() - 1], "command_finished");
        let error = event(&events, "error");
        assert_eq!(error["code"], serde_json::json!(CODE_UPGRADE_FAILED));
        assert_eq!(
            events.last().expect("finished")["exit_code"],
            serde_json::json!(1)
        );
        assert!(err.contains(CODE_UPGRADE_FAILED), "{err}");
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
        assert!(err_text.contains("recovery: rerun"), "{err_text}");
        for banned in ["--apply", "dx env", "dx codegen", "dx rollback", "dx setup"] {
            assert!(
                !err_text.contains(banned),
                "check run named {banned}: {err_text}"
            );
        }
    }

    #[test]
    fn upgrade_apply_recovery_names_surviving_commands_and_keeps_workspace() {
        let scratch = dx_test_scratch::scratch("dx-adopt-upgrade-recovery-");
        let root = scratch.path().to_path_buf();
        let workspace = format!("--workspace={}", root.display());
        let inv = invocation(&[
            "upgrade",
            "--apply",
            "--from=1.2.3",
            "--to=2.0.0",
            &workspace,
        ]);
        let (code, _out, err) = run(&inv, &root);
        assert_eq!(code, 1);
        assert!(
            err.contains("dx upgrade --from 1.2.3 --to 2.0.0 --apply"),
            "{err}"
        );
        assert!(err.contains("dx rollback --apply"), "{err}");
        assert!(err.contains("dx env --apply"), "{err}");
        assert!(err.contains("dx codegen --apply"), "{err}");
        assert!(err.contains("--workspace"), "{err}");
        for banned in [
            "dx setup",
            "dx version --pin",
            "dx version --rollback",
            "--apply --apply",
        ] {
            assert!(!err.contains(banned), "recovery named {banned}: {err}");
        }
    }

    #[test]
    fn failed_upgrade_apply_leaves_agents_md_untouched() {
        let scratch = dx_test_scratch::scratch("dx-adopt-upgrade-agents-");
        let root = scratch.path().to_path_buf();
        let old = "# Mine\n<!-- dx:begin -->\n- old\n<!-- dx:end -->\nTail.\n";
        std::fs::write(root.join("AGENTS.md"), old).expect("seed");
        let inv = invocation(&["upgrade", "--apply", "--from=1.2.3", "--to=2.0.0"]);
        let (code, out, _err) = run(&inv, &root);
        assert_eq!(code, 1);
        assert!(!out.contains("wrote AGENTS.md"), "{out}");
        assert_eq!(
            std::fs::read_to_string(root.join("AGENTS.md")).expect("read"),
            old
        );
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
    }

    #[test]
    fn upgrade_rejects_downgrade_pre_exec() {
        let inv = invocation(&["upgrade", "--from=2.0.0", "--to=1.0.0"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-upgrade-downgrade-");
        let root = scratch.path().to_path_buf();
        let (code, _out, err) = run(&inv, &root);
        assert_eq!(code, pre_exec_code());
        assert!(err.contains("upgrade-only"));
    }
}
