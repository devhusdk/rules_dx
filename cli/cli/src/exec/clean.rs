use super::common::*;
use crate::args::Invocation;
use dx_clean::{
    apply_plan, collect_inventory_with_scan, measure_prune_bytes, render_prune_listing,
    UnobservedPolicy,
};
use dx_output::{
    command_finished, command_started, notice_event, operation_event, write_event, FinishedCounts,
    NoticeEvent, OutputMode,
};

pub(crate) fn execute_clean(invocation: &Invocation, env: Env<'_>) -> i32 {
    let Env {
        workspace,
        out,
        err,
        ..
    } = env;
    let json = invocation.output == OutputMode::Json;
    let apply = invocation.applies();
    let mode = if apply { "default" } else { "check" };
    if json {
        if let Ok(event) = command_started(invocation.command.name(), mode) {
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
    if !apply {
        return check_clean(out, &plan, &bytes, json, verbose);
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
    if json {
        let _ = write_event(out, &command_finished(0, &FinishedCounts::default()));
    }
    0
}

fn check_clean(
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
        let _ = writeln!(out, "{}", render_prune_listing(plan, bytes));
        let _ = writeln!(out, "run `dx clean --apply` to prune it");
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
    use dx_atomic_fs::lease;
    use dx_setup::{read_current_pair, setup_hex};

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
    fn clean_apply_preserves_leased_stale_generation() {
        let harness = Harness::new("clean-apply-leased");
        let stale = commit_clean_pair(&harness, '3', '4');
        let current = commit_clean_pair(&harness, '1', '2');
        let dx_dir = harness.workspace.join(".dx");
        let leased_hex = "4".repeat(64);
        let _leased = lease::acquire_shared(
            &dx_dir,
            lease::GenerationUse::Generated,
            &leased_hex,
            std::time::Duration::from_secs(10),
        )
        .expect("hold a reader lease on the stale generation");
        let (code, out, err) = harness.run(&["clean", "--apply"]);
        assert_eq!(code, 0, "{out}{err}");
        assert_eq!(err, "", "{err}");
        assert!(
            out.contains("pruned 1 setup records and 1 generations"),
            "{out}"
        );
        assert!(
            dx_dir.join("generated").join(&leased_hex).exists(),
            "a leased generation survives apply"
        );
        assert!(
            !dx_dir.join("environments").join("3".repeat(64)).exists(),
            "the unleased stale generation prunes"
        );
        assert!(
            !dx_dir.join("setups").join(&stale).exists(),
            "the stale setup record prunes"
        );
        assert!(
            dx_dir.join("setups").join(&current).exists(),
            "the current record survives"
        );
    }

    #[test]
    fn clean_rejects_bazel_flag_without_launch_or_deletion() {
        let harness = Harness::new("clean-bazel-rejected");
        let stale = commit_clean_pair(&harness, '3', '4');
        let _current = commit_clean_pair(&harness, '1', '2');
        for words in [
            &["clean", "--bazel"][..],
            &["clean", "--apply", "--bazel"][..],
        ] {
            let (code, out, err) = harness.run(words);
            assert_eq!(code, 2, "{out}{err}");
            assert!(err.contains("bazel clean"), "{err}");
        }
        assert!(
            harness.seen_env.borrow().is_empty(),
            "a rejected flag never launches Bazel"
        );
        assert!(
            harness.workspace.join(".dx/setups").join(&stale).exists(),
            "a rejected flag prunes nothing"
        );
    }

    #[test]
    fn clean_malformed_current_fails_closed_without_pruning() {
        let harness = Harness::new("clean-bad-current");
        let stale = commit_clean_pair(&harness, '3', '4');
        let pointer = harness.workspace.join(".dx/setups/current");
        dx_test_scratch::remove_directory_link(&pointer).expect("remove pointer");
        std::fs::write(&pointer, "not a symlink").expect("file pointer");
        for args in [&["clean"][..], &["clean", "--apply"][..]] {
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
        let (code, out, err) = harness.run(&["clean"]);
        assert_eq!(code, 0, "{out}{err}");
        let (code, out, err) = harness.run(&["clean", "--apply"]);
        assert_eq!(code, 0, "{out}{err}");
        assert!(setups.join("notes").exists(), "unmanaged paths survive");
        assert!(environments.join("README").exists());
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
