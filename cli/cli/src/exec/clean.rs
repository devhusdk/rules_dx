use super::common::*;
use crate::args::Invocation;
use dx_clean::{
    apply_plan, bazel_forward_argv, collect_inventory_with_scan, measure_prune_bytes,
    render_dry_run, UnobservedPolicy, RECOVERY_GUIDANCE,
};
use dx_output::{
    command_finished, command_started, error_event, notice_event, operation_event, write_event,
    FinishedCounts, NoticeEvent, OutputMode,
};

pub(crate) fn execute_clean(invocation: &Invocation, env: Env<'_>) -> i32 {
    let Env {
        workspace,
        runner,
        out,
        err,
        ..
    } = env;
    let json = invocation.output == OutputMode::Json;
    let apply = invocation.applies();
    let mode = if apply { "default" } else { "check" };
    if json {
        if let Ok(event) = command_started(invocation.command.name(), invocation.dry_run, mode) {
            let _ = write_event(out, &event);
        }
        if let Ok(event) = operation_event(invocation.command.name(), "collect", None) {
            let _ = write_event(out, &event);
        }
    }
    let unobserved = if invocation.prune_unobserved {
        UnobservedPolicy::PruneAcknowledged
    } else {
        UnobservedPolicy::Preserve
    };
    let inventory = match collect_inventory_with_scan(workspace, unobserved) {
        Ok(inventory) => inventory,
        Err(error) => {
            return operational(invocation, out, err, CODE_CLEAN_FAILED, &error.to_string());
        }
    };
    let plan = inventory.plan();
    let bytes = match measure_prune_bytes(workspace, &plan) {
        Ok(bytes) => bytes,
        Err(error) => {
            return operational(invocation, out, err, CODE_CLEAN_FAILED, &error.to_string());
        }
    };
    let verbose = invocation.chatty();
    if invocation.dry_run {
        if json {
            emit_clean_notices(out, &plan, &bytes, true);
            let _ = write_event(out, &command_finished(0, &FinishedCounts::default()));
            return 0;
        }
        if verbose {
            let _ = writeln!(out, "{}", render_dry_run(&plan, &bytes));
            if invocation.bazel_clean {
                let _ = writeln!(out, "would forward: bazel clean");
            }
        }
        return 0;
    }
    if !apply {
        return check_clean(invocation, out, &plan, &bytes, json, verbose);
    }
    let outcome = match apply_plan(workspace, &plan) {
        Ok(outcome) => outcome,
        Err(error) => {
            return operational(invocation, out, err, CODE_CLEAN_FAILED, &error.to_string());
        }
    };
    if json {
        emit_clean_pruned(out, &outcome, &bytes);
    } else if verbose {
        if outcome.removed_setup_records.is_empty() && outcome.removed_generations.is_empty() {
            let _ = writeln!(out, "dx clean: nothing to prune");
        } else {
            let _ = writeln!(
                out,
                "dx clean: pruned {} setup records and {} generations ({} bytes reclaimed){}",
                outcome.removed_setup_records.len(),
                outcome.removed_generations.len(),
                bytes.reclaimed(&outcome),
                if outcome.skipped_leased_generations.is_empty() {
                    String::new()
                } else {
                    format!(
                        "; preserved {} leased generations",
                        outcome.skipped_leased_generations.len()
                    )
                }
            );
        }
    }
    if !invocation.bazel_clean {
        if json {
            let _ = write_event(out, &command_finished(0, &FinishedCounts::default()));
        }
        return 0;
    }
    let mut argv = vec!["bazel".to_owned()];
    argv.extend(invocation.bazel_startup_options.iter().cloned());
    argv.extend(bazel_forward_argv());
    let bazel_code = match run_bazel(invocation, out, err, workspace, runner, &argv, &[]) {
        Ok(code) => code,
        Err(exit) => return exit,
    };
    if json {
        if bazel_code != 0 {
            if let Ok(event) = error_event(
                "bazel_failed",
                &format!(
                    "Bazel clean forward failed with exit {bazel_code} (see stderr diagnostics)"
                ),
                None,
                None,
                Some("execute"),
            ) {
                let _ = write_event(out, &event);
            }
        }
        let _ = write_event(
            out,
            &command_finished(bazel_code, &FinishedCounts::default()),
        );
        return bazel_code;
    }
    if verbose {
        let _ = writeln!(out, "{}", RECOVERY_GUIDANCE);
    }
    bazel_code
}

fn check_clean(
    invocation: &Invocation,
    out: &mut dyn std::io::Write,
    plan: &dx_clean::CleanPlan,
    bytes: &dx_clean::PruneBytes,
    json: bool,
    verbose: bool,
) -> i32 {
    let drift = !plan.prune_setup_records.is_empty() || !plan.prune_generations.is_empty();
    if !drift {
        if json {
            let _ = write_event(out, &command_finished(0, &FinishedCounts::default()));
            return 0;
        }
        if verbose {
            let _ = writeln!(out, "dx clean: nothing to prune");
        }
        return 0;
    }
    if json {
        emit_clean_notices(out, plan, bytes, true);
        let _ = write_event(out, &command_finished(1, &FinishedCounts::default()));
        return 1;
    }
    if verbose {
        let _ = writeln!(out, "{}", render_dry_run(plan, bytes));
        if invocation.bazel_clean {
            let _ = writeln!(out, "run `dx clean --apply --bazel` to prune it");
            let _ = writeln!(out, "would forward: bazel clean");
        } else {
            let _ = writeln!(out, "run `dx clean --apply` to prune it");
        }
    }
    1
}

fn emit_clean_notices(
    out: &mut dyn std::io::Write,
    plan: &dx_clean::CleanPlan,
    bytes: &dx_clean::PruneBytes,
    planned: bool,
) {
    let code = if planned {
        "clean_planned"
    } else {
        "clean_pruned"
    };
    for hex in &plan.prune_setup_records {
        let size = bytes
            .setup_record_bytes
            .iter()
            .find(|(entry, _)| entry == hex)
            .map(|(_, size)| *size)
            .unwrap_or(0);
        let path = format!(".dx/setups/{hex}");
        let message = if planned {
            format!("would prune setup record {path} ({size} bytes)")
        } else {
            format!("pruned setup record {path} ({size} bytes)")
        };
        if let Ok(event) = notice_event(&NoticeEvent {
            level: "info".to_owned(),
            code: code.to_owned(),
            message,
            related_command: Some("clean".to_owned()),
            scope: None,
            path: Some(path),
            language: None,
            import: None,
        }) {
            let _ = write_event(out, &event);
        }
    }
    for generation in &plan.prune_generations {
        let size = bytes
            .generation_bytes
            .iter()
            .find(|(entry, _)| entry == generation)
            .map(|(_, size)| *size)
            .unwrap_or(0);
        let path = format!(".dx/{}/{}", generation.kind.dir_name(), generation.hex);
        let message = if planned {
            format!("would prune generation {path} ({size} bytes)")
        } else {
            format!("pruned generation {path} ({size} bytes)")
        };
        if let Ok(event) = notice_event(&NoticeEvent {
            level: "info".to_owned(),
            code: code.to_owned(),
            message,
            related_command: Some("clean".to_owned()),
            scope: None,
            path: Some(path),
            language: None,
            import: None,
        }) {
            let _ = write_event(out, &event);
        }
    }
    for hex in &plan.preserved_unobserved_setup_records {
        let path = format!(".dx/setups/{hex}");
        if let Ok(event) = notice_event(&NoticeEvent {
            level: "info".to_owned(),
            code: "clean_preserved_unobserved".to_owned(),
            message: format!("preserved unobserved setup record {path} (observation unavailable)"),
            related_command: Some("clean".to_owned()),
            scope: None,
            path: Some(path),
            language: None,
            import: None,
        }) {
            let _ = write_event(out, &event);
        }
    }
    for generation in &plan.preserved_unobserved_generations {
        let path = format!(".dx/{}/{}", generation.kind.dir_name(), generation.hex);
        if let Ok(event) = notice_event(&NoticeEvent {
            level: "info".to_owned(),
            code: "clean_preserved_unobserved".to_owned(),
            message: format!("preserved unobserved generation {path} (observation unavailable)"),
            related_command: Some("clean".to_owned()),
            scope: None,
            path: Some(path),
            language: None,
            import: None,
        }) {
            let _ = write_event(out, &event);
        }
    }
}

fn emit_clean_pruned(
    out: &mut dyn std::io::Write,
    outcome: &dx_clean::CleanOutcome,
    bytes: &dx_clean::PruneBytes,
) {
    for hex in &outcome.removed_setup_records {
        let size = bytes
            .setup_record_bytes
            .iter()
            .find(|(entry, _)| entry == hex)
            .map(|(_, size)| *size)
            .unwrap_or(0);
        let path = format!(".dx/setups/{hex}");
        if let Ok(event) = notice_event(&NoticeEvent {
            level: "info".to_owned(),
            code: "clean_pruned".to_owned(),
            message: format!("pruned setup record {path} ({size} bytes)"),
            related_command: Some("clean".to_owned()),
            scope: None,
            path: Some(path),
            language: None,
            import: None,
        }) {
            let _ = write_event(out, &event);
        }
    }
    for generation in &outcome.removed_generations {
        let size = bytes
            .generation_bytes
            .iter()
            .find(|(entry, _)| entry == generation)
            .map(|(_, size)| *size)
            .unwrap_or(0);
        let path = format!(".dx/{}/{}", generation.kind.dir_name(), generation.hex);
        if let Ok(event) = notice_event(&NoticeEvent {
            level: "info".to_owned(),
            code: "clean_pruned".to_owned(),
            message: format!("pruned generation {path} ({size} bytes)"),
            related_command: Some("clean".to_owned()),
            scope: None,
            path: Some(path),
            language: None,
            import: None,
        }) {
            let _ = write_event(out, &event);
        }
    }
    for generation in &outcome.skipped_leased_generations {
        let path = format!(".dx/{}/{}", generation.kind.dir_name(), generation.hex);
        if let Ok(event) = notice_event(&NoticeEvent {
            level: "info".to_owned(),
            code: "clean_preserved_leased".to_owned(),
            message: format!("preserved leased generation {path} (a live reader holds it)"),
            related_command: Some("clean".to_owned()),
            scope: None,
            path: Some(path),
            language: None,
            import: None,
        }) {
            let _ = write_event(out, &event);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::*;
    use dx_setup::{read_current_pair, setup_hex};

    #[test]
    fn clean_dry_run_empty_workspace_reports_nothing() {
        let harness = Harness::new("clean-dryrun-empty");
        let (code, out, err) = harness.run(&["clean", "--dry-run"]);
        assert_eq!(code, 0, "{out}{err}");
        assert!(out.contains("nothing to prune"), "{out}");
        assert_eq!(err, "", "{err}");
        assert!(
            harness.seen_env.borrow().is_empty(),
            "dry-run launches nothing"
        );
        assert!(
            !harness.workspace.join(".dx").exists(),
            "dry-run creates no managed state"
        );
    }

    #[test]
    fn clean_dry_run_lists_stale_record_and_deletes_nothing() {
        let harness = Harness::new("clean-dryrun-list");
        let stale = commit_clean_pair(&harness, '3', '4');
        let current = commit_clean_pair(&harness, '1', '2');
        let (code, out, err) = harness.run(&["clean", "--dry-run"]);
        assert_eq!(code, 0, "{out}{err}");
        assert!(
            out.contains(&format!("prune setup record: .dx/setups/{stale}")),
            "{out}"
        );
        assert!(out.contains("bytes)"), "{out}");
        assert!(out.contains("reclaimable total:"), "{out}");
        assert!(
            out.contains(&format!("preserve current: .dx/setups/{current}")),
            "{out}"
        );
        assert!(
            !out.contains(&format!("prune setup record: .dx/setups/{current}")),
            "{out}"
        );
        assert!(
            harness.workspace.join(".dx/setups").join(&stale).exists(),
            "dry-run deletes nothing"
        );
        assert!(harness.seen_env.borrow().is_empty());
    }

    #[test]
    fn clean_apply_prunes_stale_record_and_orphaned_generations() {
        let harness = Harness::new("clean-apply");
        let stale = commit_clean_pair(&harness, '3', '4');
        let current = commit_clean_pair(&harness, '1', '2');
        let (code, out, err) = harness.run(&["clean", "--apply"]);
        assert_eq!(code, 0, "{out}{err}");
        assert!(
            out.contains("pruned 1 setup records and 2 generations"),
            "{out}"
        );
        assert!(out.contains("bytes reclaimed"), "{out}");
        let dx_dir = harness.workspace.join(".dx");
        assert!(!dx_dir.join("setups").join(&stale).exists());
        assert!(dx_dir.join("setups").join(&current).exists());
        assert!(!dx_dir
            .join("environments")
            .join('3'.to_string().repeat(64))
            .exists());
        assert!(!dx_dir
            .join("generated")
            .join('4'.to_string().repeat(64))
            .exists());
        assert!(
            dx_dir
                .join("environments")
                .join('1'.to_string().repeat(64))
                .exists(),
            "current generations survive"
        );
        assert_eq!(
            read_current_pair(&harness.workspace).expect("read current"),
            read_current_pair(&harness.workspace).expect("reread current"),
            "selection read is stable"
        );
        let live = read_current_pair(&harness.workspace).expect("live pair");
        let live_pair = live.expect("current still selected");
        assert_eq!(setup_hex(&live_pair), current);
    }

    #[test]
    fn clean_apply_preserves_generations_shared_with_current() {
        let harness = Harness::new("clean-apply-shared");
        commit_clean_pair(&harness, '3', '2');
        commit_clean_pair(&harness, '1', '2');
        let (code, out, err) = harness.run(&["clean", "--apply"]);
        assert_eq!(code, 0, "{out}{err}");
        assert!(
            out.contains("pruned 1 setup records and 1 generations"),
            "{out}"
        );
        assert!(out.contains("bytes reclaimed"), "{out}");
        assert!(
            harness
                .workspace
                .join(".dx/generated")
                .join('2'.to_string().repeat(64))
                .exists(),
            "generation shared with current survives"
        );
    }

    #[test]
    fn clean_bazel_forward_runs_after_prune_and_propagates_code() {
        let harness = Harness::new("clean-bazel");
        let (code, out, err) = harness.run(&["clean", "--apply", "--bazel"]);
        assert_eq!(code, 0, "{out}{err}");
        assert!(out.contains("nothing to prune"), "{out}");
        assert!(out.contains("re-run `dx setup`"), "{out}");
        assert_eq!(
            harness.seen_env.borrow().len(),
            1,
            "exactly one Bazel launch for the explicit forward"
        );

        let mut failing = Harness::new("clean-bazel-fail");
        failing.bazel_code = 3;
        let (code, _, err) = failing.run(&["clean", "--apply", "--bazel"]);
        assert_eq!(code, 3, "{err}");

        let dry = Harness::new("clean-bazel-dryrun");
        let (code, out, err) = dry.run(&["clean", "--dry-run", "--bazel"]);
        assert_eq!(code, 0, "{out}{err}");
        assert!(out.contains("would forward: bazel clean"), "{out}");
        assert!(
            dry.seen_env.borrow().is_empty(),
            "dry-run lists the forward without launching"
        );
    }

    #[test]
    fn clean_malformed_current_fails_closed_without_pruning() {
        let harness = Harness::new("clean-bad-current");
        let stale = commit_clean_pair(&harness, '3', '4');
        let pointer = harness.workspace.join(".dx/setups/current");
        dx_test_scratch::remove_directory_link(&pointer).expect("remove pointer");
        std::fs::write(&pointer, "not a symlink").expect("file pointer");
        for args in [&["clean", "--dry-run"][..], &["clean"][..]] {
            let (code, out, err) = harness.run(args);
            assert_eq!(code, 1, "{out}{err}");
            let combined = format!("{out}{err}");
            assert!(combined.contains("clean_failed"), "{combined}");
        }
        assert!(
            harness.workspace.join(".dx/setups").join(&stale).exists(),
            "failure prunes nothing"
        );
    }

    #[test]
    fn clean_unmanaged_paths_are_refused_and_preserved() {
        let harness = Harness::new("clean-unmanaged");
        let setups = harness.workspace.join(".dx/setups");
        std::fs::create_dir_all(setups.join("notes")).expect("unmanaged record");
        let environments = harness.workspace.join(".dx/environments");
        std::fs::create_dir_all(&environments).expect("environments dir");
        std::fs::write(environments.join("README"), "operator notes").expect("unmanaged file");
        let (code, out, err) = harness.run(&["clean", "--dry-run"]);
        assert_eq!(code, 0, "{out}{err}");
        assert!(out.contains("refuse unmanaged path: README"), "{out}");
        assert!(out.contains("refuse unmanaged path: notes"), "{out}");
        let (code, out, err) = harness.run(&["clean"]);
        assert_eq!(code, 0, "{out}{err}");
        assert!(out.contains("nothing to prune"), "{out}");
        assert!(setups.join("notes").exists(), "unmanaged paths survive");
        assert!(environments.join("README").exists());
    }

    #[test]
    fn clean_dry_run_json_streams_planning_events() {
        let harness = Harness::new("clean-dry-json");
        let stale = commit_clean_pair(&harness, '3', '4');
        let _current = commit_clean_pair(&harness, '1', '2');
        let (code, out, err) = harness.run(&["clean", "--dry-run", "--output=json"]);
        assert_eq!(code, 0, "{out}{err}");
        let events = json_events(&out);
        let kinds = event_kinds(&events);
        assert_eq!(kinds[0], "command_started");
        assert!(kinds.contains(&"operation"), "{kinds:?}");
        assert!(kinds.contains(&"notice"), "{kinds:?}");
        assert_eq!(kinds[kinds.len() - 1], "command_finished");
        let op = event(&events, "operation");
        assert_eq!(op["phase"], serde_json::json!("collect"));
        let notice = event(&events, "notice");
        assert_eq!(notice["code"], serde_json::json!("clean_planned"));
        assert!(notice["message"]
            .as_str()
            .expect("message")
            .contains(&stale));
        assert_eq!(
            events.last().expect("finished")["exit_code"],
            serde_json::json!(0)
        );
        assert_eq!(err, "", "{err}");
    }

    #[test]
    fn clean_live_json_streams_pruned_notices() {
        let harness = Harness::new("clean-live-json");
        let stale = commit_clean_pair(&harness, '3', '4');
        let _current = commit_clean_pair(&harness, '1', '2');
        let (code, out, err) = harness.run(&["clean", "--apply", "--output=json"]);
        assert_eq!(code, 0, "{out}{err}");
        let events = json_events(&out);
        assert!(
            events
                .iter()
                .any(|event| event["event"] == serde_json::json!("notice")
                    && event["code"] == serde_json::json!("clean_pruned")),
            "{out}"
        );
        assert!(out.contains(&stale), "{out}");
        assert_eq!(
            events.last().expect("finished")["exit_code"],
            serde_json::json!(0)
        );
        assert_eq!(err, "", "{err}");
    }

    #[test]
    fn clean_json_failure_emits_error_and_finished() {
        let harness = Harness::new("clean-json-fail");
        let stale = commit_clean_pair(&harness, '3', '4');
        let pointer = harness.workspace.join(".dx/setups/current");
        dx_test_scratch::remove_directory_link(&pointer).expect("remove pointer");
        std::fs::write(&pointer, "not a symlink").expect("file pointer");
        let (code, out, err) = harness.run(&["clean", "--output=json"]);
        assert_eq!(code, 1, "{out}{err}");
        assert!(out.contains("clean_failed"), "{out}");
        assert!(out.contains("command_finished"), "{out}");
        assert!(err.contains("clean_failed"), "{err}");
        assert!(
            harness.workspace.join(".dx/setups").join(&stale).exists(),
            "failure prunes nothing"
        );
    }

    #[test]
    fn clean_default_reports_drift_without_pruning() {
        let harness = Harness::new("clean-check-drift");
        let stale = commit_clean_pair(&harness, '3', '4');
        let current = commit_clean_pair(&harness, '1', '2');
        let (code, out, err) = harness.run(&["clean"]);
        assert_eq!(code, 1, "{out}{err}");
        assert!(
            out.contains(&format!("prune setup record: .dx/setups/{stale}")),
            "{out}"
        );
        assert!(out.contains("run `dx clean --apply` to prune it"), "{out}");
        assert_eq!(err, "", "{err}");
        assert!(
            harness.workspace.join(".dx/setups").join(&stale).exists(),
            "check prunes nothing"
        );
        assert!(
            harness.workspace.join(".dx/setups").join(&current).exists(),
            "check keeps current"
        );
        assert!(
            harness.seen_env.borrow().is_empty(),
            "check launches nothing"
        );
    }

    #[test]
    fn clean_default_clean_workspace_succeeds() {
        let harness = Harness::new("clean-check-clean");
        let (code, out, err) = harness.run(&["clean"]);
        assert_eq!(code, 0, "{out}{err}");
        assert!(out.contains("nothing to prune"), "{out}");
        assert_eq!(err, "", "{err}");
    }

    #[test]
    fn clean_explicit_check_matches_default() {
        let default = Harness::new("clean-check-matches-default");
        let stale = commit_clean_pair(&default, '3', '4');
        let _current = commit_clean_pair(&default, '1', '2');
        let (default_code, default_out, _) = default.run(&["clean"]);
        let explicit = Harness::new("clean-check-matches-explicit");
        let estale = commit_clean_pair(&explicit, '3', '4');
        let _ecurrent = commit_clean_pair(&explicit, '1', '2');
        assert_eq!(stale, estale, "fixtures agree");
        let (explicit_code, explicit_out, _) = explicit.run(&["clean", "--check"]);
        assert_eq!(default_code, explicit_code);
        assert_eq!(default_out, explicit_out);
        assert_eq!(default_code, 1, "{default_out}");
        assert!(default_out.contains("run `dx clean --apply` to prune it"));
    }

    #[test]
    fn clean_check_with_bazel_reports_without_forwarding() {
        let harness = Harness::new("clean-check-bazel");
        let stale = commit_clean_pair(&harness, '3', '4');
        let _current = commit_clean_pair(&harness, '1', '2');
        let (code, out, err) = harness.run(&["clean", "--bazel"]);
        assert_eq!(code, 1, "{out}{err}");
        assert!(out.contains(&stale), "{out}");
        assert!(
            out.contains("run `dx clean --apply --bazel` to prune it"),
            "{out}"
        );
        assert!(out.contains("would forward: bazel clean"), "{out}");
        assert!(
            harness.seen_env.borrow().is_empty(),
            "check never forwards bazel clean"
        );
        assert!(
            harness.workspace.join(".dx/setups").join(&stale).exists(),
            "check prunes nothing"
        );
    }

    #[test]
    fn clean_check_json_emits_planned_notices_with_drift_exit() {
        let harness = Harness::new("clean-check-json-drift");
        let stale = commit_clean_pair(&harness, '3', '4');
        let _current = commit_clean_pair(&harness, '1', '2');
        let (code, out, err) = harness.run(&["clean", "--output=json"]);
        assert_eq!(code, 1, "{out}{err}");
        let events = json_events(&out);
        let kinds = event_kinds(&events);
        assert_eq!(kinds[0], "command_started");
        assert!(kinds.contains(&"operation"), "{kinds:?}");
        assert!(kinds.contains(&"notice"), "{kinds:?}");
        assert_eq!(kinds[kinds.len() - 1], "command_finished");
        let started = event(&events, "command_started");
        assert_eq!(started["mode"], serde_json::json!("check"));
        let notice = event(&events, "notice");
        assert_eq!(notice["code"], serde_json::json!("clean_planned"));
        let message = notice["message"].as_str().expect("message");
        assert!(message.contains(&stale));
        assert_eq!(
            events.last().expect("finished")["exit_code"],
            serde_json::json!(1)
        );
        assert_eq!(err, "", "{err}");
        assert!(
            harness.workspace.join(".dx/setups").join(&stale).exists(),
            "json check prunes nothing"
        );
    }
}
