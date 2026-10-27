use std::io::Write;
use std::path::Path;

use crate::exec::common::{check_stdout_write, emit_started};
use dx_plan::args::Invocation;

use dx_output::OutputMode;

use super::rollback::json_pin;
use super::status::CODE_STATUS_PIN_MISMATCH;
use super::{agents, operational, pre_exec, summaries_suppressed};

pub(crate) const CODE_UPGRADE_FAILED: &str = "upgrade_failed";

/// Reports one failure; JSON output opens the stream first so the envelope stays whole.
fn fail(
    invocation: &Invocation,
    out: &mut dyn Write,
    err: &mut dyn Write,
    code: &str,
    message: &str,
) -> i32 {
    if invocation.output == OutputMode::Json {
        if let Err(exit) = emit_started(invocation, out) {
            return exit;
        }
    }
    operational(invocation, out, err, code, message)
}

/// Pins `--to` (this binary's own version) over a `--from` pin, records the move for
/// `dx rollback`, then refreshes existing launchers and an existing AGENTS.md block.
/// Every refusal runs before the first write.
pub(crate) fn execute_upgrade(
    invocation: &Invocation,
    workspace: &Path,
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
    let refuse = |out: &mut dyn Write, err: &mut dyn Write, message: &str| {
        fail(
            invocation,
            out,
            err,
            CODE_UPGRADE_FAILED,
            &format!("upgrade refused: {message} (nothing was written)"),
        )
    };
    if !dx_adopt::version_pin_matches_module(to, dx_adopt::MODULE_VERSION) {
        let message = format!(
            "this dx is {}, not {to}; run the {to} binary: `bazel run @rules_dx//:dx -- upgrade --from {from} --to {to} --apply`",
            dx_adopt::MODULE_VERSION,
        );
        return refuse(out, err, &message);
    }
    let pin = match dx_adopt::read_version_pin(workspace) {
        Ok(pin) if !pin.is_empty() => pin,
        Ok(_) => return refuse(out, err, ".dx/version is empty; run `dx init --apply`"),
        Err(_) if !workspace.join(".dx/version").is_file() => {
            return refuse(out, err, "no .dx/version; run `dx init --apply`")
        }
        Err(error) => return refuse(out, err, &error.to_string()),
    };
    let rerun = dx_adopt::version_pin_matches_module(&pin, to);
    if !rerun && !dx_adopt::version_pin_matches_module(&pin, from) {
        let message = format!(
            "pin {pin} is neither --from {from} nor --to {to}; fix: {}",
            dx_adopt::pin_fix_hint(&pin, dx_adopt::MODULE_VERSION)
        );
        return refuse(out, err, &message);
    }
    let is_json = invocation.output == OutputMode::Json;
    if !invocation.applies() {
        if rerun {
            if is_json {
                return json_pin(invocation, out, "check", to).map_or_else(|exit| exit, |()| 0);
            }
            if !summaries_suppressed(invocation) {
                if let Err(exit) = check_stdout_write(writeln!(out, "upgrade current: {to}")) {
                    return exit;
                }
            }
            return 0;
        }
        let retry = dx_adopt::plan_upgrade(
            from,
            to,
            &dx_adopt::UpgradeIntent {
                apply: true,
                ..intent
            },
        )
        .map(|plan| plan.retry_command)
        .unwrap_or_else(|_| plan.retry_command.clone());
        let message = format!("pin {pin} can upgrade to {to} (run `{retry}`)");
        return fail(invocation, out, err, CODE_STATUS_PIN_MISMATCH, &message);
    }
    if let Err(error) = dx_adopt::plan_launcher_files(workspace, to, false) {
        return refuse(out, err, &error.to_string());
    }
    let landed = |out: &mut dyn Write, err: &mut dyn Write, what: &str| {
        let message = format!("pinned {to} but {what}; {}", plan.message);
        fail(invocation, out, err, CODE_UPGRADE_FAILED, &message)
    };
    if !rerun {
        if let Err(error) = dx_adopt::write_version_pin(workspace, to) {
            return refuse(out, err, &error.to_string());
        }
        if let Err(error) = dx_adopt::record_pin_operation(workspace, Some(&pin), Some(to)) {
            return landed(out, err, &format!("history unavailable: {error}"));
        }
    }
    let mut wrote = match dx_adopt::refresh_launcher_files(workspace, to) {
        Ok(wrote) => wrote,
        Err(error) => return landed(out, err, &format!("launchers not refreshed: {error}")),
    };
    match agents::write(workspace, false) {
        Ok(true) => wrote.push("AGENTS.md".to_owned()),
        Ok(false) => {}
        Err(error) => return landed(out, err, &format!("AGENTS.md not refreshed: {error}")),
    }
    if is_json {
        return json_pin(invocation, out, "default", to).map_or_else(|exit| exit, |()| 0);
    }
    let line = if rerun {
        format!("pinned {to} (upgrade current)")
    } else {
        format!("pinned {to} (upgrade from {pin})")
    };
    if let Err(exit) = check_stdout_write(writeln!(out, "{line}")) {
        return exit;
    }
    if !summaries_suppressed(invocation) {
        for rel in wrote {
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
    use crate::adopt::test_support::{event, event_kinds, invocation, json_events, run};
    use dx_plan::skew::{disposition, is_skewed, SkewDisposition};
    use dx_process::pre_exec_code;
    use std::path::PathBuf;

    const OLD: &str = "0.0.0-alpha.1";
    const STALE_AGENTS: &str = "# Mine\n<!-- dx:begin -->\n- old\n<!-- dx:end -->\nTail.\n";

    fn module() -> &'static str {
        dx_adopt::MODULE_VERSION
    }

    fn pinned(prefix: &str, pin: &str) -> (impl Sized, PathBuf) {
        let scratch = dx_test_scratch::scratch(prefix);
        let root = scratch.path().to_path_buf();
        dx_adopt::write_version_pin(&root, pin).expect("pin");
        (scratch, root)
    }

    fn read_pin(root: &Path) -> String {
        dx_adopt::read_version_pin(root).expect("pin")
    }

    fn snapshot(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
        let mut files = Vec::new();
        let mut stack = vec![root.to_path_buf()];
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir).expect("dir") {
                let path = entry.expect("entry").path();
                if path.is_dir() {
                    stack.push(path);
                } else {
                    files.push((path.clone(), std::fs::read(&path).expect("file")));
                }
            }
        }
        files.sort();
        files
    }

    fn upgrade(extra: &[&str]) -> Invocation {
        let from = format!("--from={OLD}");
        let to = format!("--to={}", module());
        let mut words = vec!["upgrade", from.as_str(), to.as_str()];
        words.extend_from_slice(extra);
        invocation(&words)
    }

    #[test]
    fn the_skew_hint_upgrades_the_pin_and_refreshes_existing_files() {
        let (_scratch, root) = pinned("dx-adopt-upgrade-hint-", OLD);
        assert!(is_skewed(&read_pin(&root)));
        std::fs::write(root.join("AGENTS.md"), STALE_AGENTS).expect("agents");
        dx_adopt::write_launcher_files(&root, OLD, &mut dx_adopt::Applied::default())
            .expect("launchers");
        std::fs::remove_file(root.join("tools/dx.cmd")).expect("drop cmd");
        let hint = dx_adopt::pin_fix_hint(OLD, module());
        let words: Vec<&str> = hint.split_whitespace().skip(1).collect();
        assert_eq!(words[0], "upgrade", "{hint}");
        let inv = invocation(&words);
        assert_eq!(disposition(&inv, true), SkewDisposition::Proceed);
        let (code, out, err) = run(&inv, &root);
        assert_eq!(code, 0, "{out}{err}");
        assert_eq!(
            out,
            format!(
                "pinned {} (upgrade from {OLD})\nwrote tools/dx\nwrote AGENTS.md\n",
                module()
            )
        );
        assert_eq!(read_pin(&root), module());
        assert!(!is_skewed(&read_pin(&root)));
        let record = dx_adopt::read_pin_record(&root).expect("record");
        assert_eq!(record.previous.as_deref(), Some(OLD));
        assert_eq!(record.current.as_deref(), Some(module()));
        let agents = std::fs::read_to_string(root.join("AGENTS.md")).expect("agents");
        assert_ne!(agents, STALE_AGENTS);
        assert!(agents.starts_with("# Mine\n") && agents.ends_with("Tail.\n"));
        assert_eq!(
            dx_adopt::launcher_version(&root, "tools/dx").as_deref(),
            Some(module())
        );
        assert!(
            !root.join("tools/dx.cmd").exists(),
            "absent launcher created"
        );
    }

    #[test]
    fn upgrade_creates_no_launcher_or_agents_md() {
        let (_scratch, root) = pinned("dx-adopt-upgrade-absent-", OLD);
        let (code, out, err) = run(&upgrade(&["--apply"]), &root);
        assert_eq!(code, 0, "{out}{err}");
        assert!(!root.join("tools").exists());
        assert!(!root.join("AGENTS.md").exists());
    }

    #[test]
    fn upgrade_runs_on_skew() {
        assert_eq!(
            disposition(&upgrade(&["--apply"]), true),
            SkewDisposition::Proceed
        );
    }

    #[test]
    fn check_mode_writes_nothing_and_reports_pending_or_current() {
        let (_scratch, root) = pinned("dx-adopt-upgrade-check-", OLD);
        std::fs::write(root.join("AGENTS.md"), STALE_AGENTS).expect("agents");
        let before = snapshot(&root);
        let (code, _out, err) = run(&upgrade(&[]), &root);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains(CODE_STATUS_PIN_MISMATCH), "{err}");
        assert!(
            err.contains(&format!("can upgrade to {}", module())),
            "{err}"
        );
        assert!(err.contains("--apply`"), "{err}");
        assert_eq!(snapshot(&root), before, "check must not write");
        let (_scratch, root) = pinned("dx-adopt-upgrade-current-", module());
        let (code, out, err) = run(&upgrade(&[]), &root);
        assert_eq!(code, 0, "{err}");
        assert_eq!(out, format!("upgrade current: {}\n", module()));
    }

    #[test]
    fn a_from_that_does_not_match_the_pin_is_refused() {
        let (_scratch, root) = pinned("dx-adopt-upgrade-from-", "0.0.0-alpha.2");
        let before = snapshot(&root);
        let (code, _out, err) = run(&upgrade(&["--apply"]), &root);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains(CODE_UPGRADE_FAILED), "{err}");
        assert!(err.contains("is neither --from"), "{err}");
        assert!(err.contains("nothing was written"), "{err}");
        assert_eq!(snapshot(&root), before);
    }

    #[test]
    fn a_missing_pin_points_at_init() {
        let scratch = dx_test_scratch::scratch("dx-adopt-upgrade-nopin-");
        let root = scratch.path().to_path_buf();
        let (code, _out, err) = run(&upgrade(&["--apply"]), &root);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("dx init --apply"), "{err}");
        assert!(!root.join(".dx").exists());
    }

    #[test]
    fn a_to_other_than_this_binary_is_refused() {
        let (_scratch, root) = pinned("dx-adopt-upgrade-to-", "1.2.3");
        let before = snapshot(&root);
        let inv = invocation(&["upgrade", "--apply", "--from=1.2.3", "--to=2.0.0"]);
        let (code, _out, err) = run(&inv, &root);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains(CODE_UPGRADE_FAILED), "{err}");
        assert!(
            err.contains("bazel run @rules_dx//:dx -- upgrade --from 1.2.3 --to 2.0.0 --apply"),
            "{err}"
        );
        assert_eq!(snapshot(&root), before);
    }

    #[test]
    fn a_rerun_after_apply_succeeds_and_keeps_the_history() {
        let (_scratch, root) = pinned("dx-adopt-upgrade-rerun-", OLD);
        let apply = upgrade(&["--apply"]);
        assert_eq!(run(&apply, &root).0, 0);
        let history = std::fs::read(root.join(dx_adopt::VERSION_HISTORY_REL)).expect("history");
        let (code, out, err) = run(&apply, &root);
        assert_eq!(code, 0, "{out}{err}");
        assert_eq!(
            std::fs::read(root.join(dx_adopt::VERSION_HISTORY_REL)).expect("history"),
            history
        );
    }

    #[test]
    fn rollback_restores_the_pin_upgrade_replaced() {
        let (_scratch, root) = pinned("dx-adopt-upgrade-rollback-", OLD);
        assert_eq!(run(&upgrade(&["--apply"]), &root).0, 0);
        let (code, out, err) = run(&invocation(&["rollback", "--apply"]), &root);
        assert_eq!(code, 0, "{out}{err}");
        assert_eq!(read_pin(&root), OLD);
    }

    #[test]
    fn upgrade_json_streams_the_status_envelope() {
        let (_scratch, root) = pinned("dx-adopt-upgrade-json-", OLD);
        let (code, out, _err) = run(&upgrade(&["--apply", "--output=json"]), &root);
        assert_eq!(code, 0, "{out}");
        let events = json_events(&out);
        assert_eq!(
            event_kinds(&events),
            vec!["command_started", "status", "command_finished"]
        );
        assert_eq!(events[1]["name"], "pin");
        let (_scratch, root) = pinned("dx-adopt-upgrade-json-refused-", "0.0.0-alpha.2");
        let (code, out, _err) = run(&upgrade(&["--apply", "--output=json"]), &root);
        assert_eq!(code, 1);
        let events = json_events(&out);
        assert_eq!(
            event_kinds(&events),
            vec!["command_started", "error", "command_finished"]
        );
        assert_eq!(event(&events, "error")["code"], CODE_UPGRADE_FAILED);
    }

    #[test]
    fn following_the_hint_for_a_newer_pin_clears_skew() {
        let (_scratch, root) = pinned("dx-adopt-upgrade-newer-", "9.9.9");
        let hint = dx_adopt::pin_fix_hint("9.9.9", module());
        assert_eq!(hint, format!("write {} to .dx/version", module()));
        dx_adopt::write_version_pin(&root, module()).expect("follow hint");
        assert!(!is_skewed(&read_pin(&root)));
    }

    #[test]
    fn an_orphaned_agents_marker_after_the_pin_lands_names_recovery() {
        let (_scratch, root) = pinned("dx-adopt-upgrade-orphan-", OLD);
        std::fs::write(root.join("AGENTS.md"), "# Mine\n<!-- dx:begin -->\n").expect("agents");
        let apply = upgrade(&["--apply"]);
        let (code, _out, err) = run(&apply, &root);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains(CODE_UPGRADE_FAILED), "{err}");
        assert!(
            err.contains(&format!("pinned {} but AGENTS.md not refreshed", module())),
            "{err}"
        );
        assert!(err.contains("recovery: rerun"), "{err}");
        assert_eq!(read_pin(&root), module());
        std::fs::write(root.join("AGENTS.md"), STALE_AGENTS).expect("fix");
        let (code, out, err) = run(&apply, &root);
        assert_eq!(code, 0, "{out}{err}");
        assert!(out.contains("wrote AGENTS.md"), "{out}");
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
