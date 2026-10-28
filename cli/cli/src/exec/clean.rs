use super::common::*;
use super::explain_store;
use dx_clean::{
    apply_plan, collect_inventory_with_scan_from, measure_preserved_bytes, measure_prune_bytes,
    render_prune_listing, GenerationView, PruneBytes, UnobservedPolicy, PRUNE_UNOBSERVED_FLAG,
};
use dx_output::{
    command_finished, command_started, notice_event, operation_event, FinishedCounts, NoticeEvent,
    OutputMode,
};
use dx_plan::args::Invocation;

pub(crate) fn execute_clean(invocation: &Invocation, env: Env<'_>) -> i32 {
    let Env {
        workspace,
        proc_root,
        out,
        err,
        ..
    } = env;
    let json = invocation.output == OutputMode::Json;
    let apply = invocation.applies();
    let mode = if apply { "default" } else { "check" };
    if json {
        if let Err(exit) = emit_built(out, command_started(invocation.command.name(), mode)) {
            return exit;
        }
        if let Err(exit) = emit_built(
            out,
            operation_event(invocation.command.name(), "collect", None),
        ) {
            return exit;
        }
    }
    let unobserved = if invocation.prune_unobserved {
        UnobservedPolicy::PruneAcknowledged
    } else {
        UnobservedPolicy::Preserve
    };
    let inventory = match collect_inventory_with_scan_from(workspace, proc_root, unobserved) {
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
    let preserved = match measure_preserved_bytes(workspace, &plan) {
        Ok(preserved) => preserved,
        Err(error) => {
            return operational(invocation, out, err, CODE_CLEAN_FAILED, &error.to_string());
        }
    };
    let verbose = invocation.chatty();
    if !apply {
        return check_clean(out, &plan, &bytes, &preserved, json, verbose);
    }
    let outcome = match apply_plan(workspace, &plan) {
        Ok(outcome) => outcome,
        Err(error) => {
            return operational(invocation, out, err, CODE_CLEAN_FAILED, &error.to_string());
        }
    };
    if let Err(exit) = report_applied(out, &plan, &outcome, &bytes, &preserved, json, verbose) {
        return exit;
    }
    match explain_store::clear(workspace) {
        Ok(0) => {}
        Ok(removed) => {
            if json {
                let notice = NoticeEvent {
                    related_command: Some("clean".to_owned()),
                    path: Some(".dx/explain".to_owned()),
                    ..NoticeEvent::info(
                        CODE_CLEAN_PRUNED,
                        format!("pruned {removed} execution logs .dx/explain"),
                    )
                };
                if let Err(exit) = emit_built(out, notice_event(&notice)) {
                    return exit;
                }
            } else if verbose {
                if let Err(exit) = check_stdout_write(writeln!(
                    out,
                    "dx clean: pruned {removed} execution logs from .dx/explain"
                )) {
                    return exit;
                }
            }
        }
        Err(error) => {
            return operational(
                invocation,
                out,
                err,
                CODE_CLEAN_FAILED,
                &format!("could not prune .dx/explain: {error}"),
            );
        }
    }
    if json {
        if let Err(exit) = emit_event(out, &command_finished(0, &FinishedCounts::default())) {
            return exit;
        }
    }
    0
}

fn prune_unobserved_hint() -> String {
    format!("run `dx clean --apply {PRUNE_UNOBSERVED_FLAG}` to prune it anyway")
}

fn report_applied(
    out: &mut dyn std::io::Write,
    plan: &dx_clean::CleanPlan,
    outcome: &dx_clean::CleanOutcome,
    bytes: &PruneBytes,
    preserved: &PruneBytes,
    json: bool,
    verbose: bool,
) -> Result<(), i32> {
    if json {
        emit_clean_pruned(out, outcome, bytes)?;
        return emit_preserved_unobserved(out, plan, preserved);
    }
    if !verbose {
        return Ok(());
    }
    let removed_nothing =
        outcome.removed_setup_records.is_empty() && outcome.removed_generations.is_empty();
    if removed_nothing && plan.has_preserved_unobserved() {
        check_stdout_write(writeln!(
            out,
            "dx clean: preserved {} unobserved setup records and {} unobserved generations (process observation unavailable)",
            plan.preserved_unobserved_setup_records.len(),
            plan.preserved_unobserved_generations.len(),
        ))?;
        return check_stdout_write(writeln!(out, "{}", prune_unobserved_hint()));
    }
    if removed_nothing {
        return check_stdout_write(writeln!(out, "dx clean: nothing to prune"));
    }
    check_stdout_write(writeln!(
        out,
        "dx clean: pruned {} setup records and {} generations ({} bytes reclaimed){}",
        outcome.removed_setup_records.len(),
        outcome.removed_generations.len(),
        bytes.reclaimed(outcome),
        if outcome.skipped_leased_generations.is_empty() {
            String::new()
        } else {
            format!(
                "; preserved {} leased generations",
                outcome.skipped_leased_generations.len()
            )
        }
    ))
}

fn check_clean(
    out: &mut dyn std::io::Write,
    plan: &dx_clean::CleanPlan,
    bytes: &PruneBytes,
    preserved: &PruneBytes,
    json: bool,
    verbose: bool,
) -> i32 {
    let drift = plan.has_prune();
    if !drift && plan.has_preserved_unobserved() {
        if json {
            if let Err(exit) = emit_preserved_unobserved(out, plan, preserved) {
                return exit;
            }
            if let Err(exit) = emit_event(out, &command_finished(0, &FinishedCounts::default())) {
                return exit;
            }
            return 0;
        }
        if verbose {
            if let Err(exit) =
                check_stdout_write(writeln!(out, "{}", render_prune_listing(plan, bytes)))
            {
                return exit;
            }
            if let Err(exit) = check_stdout_write(writeln!(out, "{}", prune_unobserved_hint())) {
                return exit;
            }
        }
        return 0;
    }
    if !drift {
        if json {
            if let Err(exit) = emit_event(out, &command_finished(0, &FinishedCounts::default())) {
                return exit;
            }
            return 0;
        }
        if verbose {
            if let Err(exit) = check_stdout_write(writeln!(out, "dx clean: nothing to prune")) {
                return exit;
            }
        }
        return 0;
    }
    if json {
        if let Err(exit) = emit_clean_planned(out, plan, bytes, preserved) {
            return exit;
        }
        if let Err(exit) = emit_event(out, &command_finished(1, &FinishedCounts::default())) {
            return exit;
        }
        return 1;
    }
    if verbose {
        if let Err(exit) =
            check_stdout_write(writeln!(out, "{}", render_prune_listing(plan, bytes)))
        {
            return exit;
        }
        if let Err(exit) = check_stdout_write(writeln!(out, "run `dx clean --apply` to prune it")) {
            return exit;
        }
    }
    1
}

fn emit_clean_planned(
    out: &mut dyn std::io::Write,
    plan: &dx_clean::CleanPlan,
    bytes: &PruneBytes,
    preserved: &PruneBytes,
) -> Result<(), i32> {
    emit_prune_notices(
        out,
        CODE_CLEAN_PLANNED,
        "would prune",
        &plan.prune_setup_records,
        &plan.prune_generations,
        bytes,
    )?;
    emit_preserved_unobserved(out, plan, preserved)
}

fn setup_record_size(bytes: &PruneBytes, hex: &str) -> u64 {
    bytes
        .setup_record_bytes
        .iter()
        .find(|(entry, _)| entry == hex)
        .map(|(_, size)| *size)
        .unwrap_or(0)
}

fn generation_size(bytes: &PruneBytes, generation: &GenerationView) -> u64 {
    bytes
        .generation_bytes
        .iter()
        .find(|(entry, _)| entry == generation)
        .map(|(_, size)| *size)
        .unwrap_or(0)
}

fn emit_prune_notices(
    out: &mut dyn std::io::Write,
    code: &str,
    verb: &str,
    setup_records: &[String],
    generations: &[GenerationView],
    bytes: &PruneBytes,
) -> Result<(), i32> {
    for hex in setup_records {
        let size = setup_record_size(bytes, hex);
        let path = format!(".dx/setups/{hex}");
        emit_built(
            out,
            notice_event(&NoticeEvent {
                related_command: Some("clean".to_owned()),
                path: Some(path.clone()),
                ..NoticeEvent::info(code, format!("{verb} setup record {path} ({size} bytes)"))
            }),
        )?;
    }
    for generation in generations {
        let size = generation_size(bytes, generation);
        let path = format!(".dx/{}/{}", generation.kind.dir_name(), generation.hex);
        emit_built(
            out,
            notice_event(&NoticeEvent {
                related_command: Some("clean".to_owned()),
                path: Some(path.clone()),
                ..NoticeEvent::info(code, format!("{verb} generation {path} ({size} bytes)"))
            }),
        )?;
    }
    Ok(())
}

fn emit_preserved_unobserved(
    out: &mut dyn std::io::Write,
    plan: &dx_clean::CleanPlan,
    preserved: &PruneBytes,
) -> Result<(), i32> {
    for hex in &plan.preserved_unobserved_setup_records {
        let size = setup_record_size(preserved, hex);
        let path = format!(".dx/setups/{hex}");
        emit_built(
            out,
            notice_event(&NoticeEvent {
                related_command: Some("clean".to_owned()),
                path: Some(path.clone()),
                ..NoticeEvent::info(CODE_CLEAN_PRESERVED_UNOBSERVED, format!("preserved unobserved setup record {path} (observation unavailable, {size} bytes)"))
            }),
        )?;
    }
    for generation in &plan.preserved_unobserved_generations {
        let size = generation_size(preserved, generation);
        let path = format!(".dx/{}/{}", generation.kind.dir_name(), generation.hex);
        emit_built(
            out,
            notice_event(&NoticeEvent {
                related_command: Some("clean".to_owned()),
                path: Some(path.clone()),
                ..NoticeEvent::info(CODE_CLEAN_PRESERVED_UNOBSERVED, format!("preserved unobserved generation {path} (observation unavailable, {size} bytes)"))
            }),
        )?;
    }
    Ok(())
}

fn emit_clean_pruned(
    out: &mut dyn std::io::Write,
    outcome: &dx_clean::CleanOutcome,
    bytes: &PruneBytes,
) -> Result<(), i32> {
    emit_prune_notices(
        out,
        CODE_CLEAN_PRUNED,
        "pruned",
        &outcome.removed_setup_records,
        &outcome.removed_generations,
        bytes,
    )?;
    for generation in &outcome.skipped_leased_generations {
        let size = generation_size(bytes, generation);
        let path = format!(".dx/{}/{}", generation.kind.dir_name(), generation.hex);
        emit_built(
            out,
            notice_event(&NoticeEvent {
                related_command: Some("clean".to_owned()),
                path: Some(path.clone()),
                ..NoticeEvent::info(
                    CODE_CLEAN_PRESERVED_LEASED,
                    format!(
                        "preserved leased generation {path} (a live reader holds it, {size} bytes)"
                    ),
                )
            }),
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::test_support::*;
    use dx_atomic_fs::lease;
    use dx_setup::{read_current_pair, setup_hex};

    #[test]
    fn clean_apply_preserves_unobserved_records_when_proc_is_unavailable() {
        let mut harness = Harness::new("clean-apply-unobserved");
        harness.proc_root = harness.workspace.join("no-such-proc");
        let stale = commit_clean_pair(&harness, '3', '4');
        let current = commit_clean_pair(&harness, '1', '2');
        let (code, out, err) = harness.run(&["clean", "--apply"]);
        assert_eq!(code, 0, "{out}{err}");
        let dx_dir = harness.workspace.join(".dx");
        assert!(
            dx_dir.join("setups").join(&stale).exists(),
            "unobserved stale record is preserved: {out}{err}"
        );
        assert!(dx_dir.join("setups").join(&current).exists());
    }

    #[test]
    fn clean_apply_prunes_stale_record_and_orphaned_generations() {
        let harness = Harness::new("clean-apply");
        let stale = commit_clean_pair(&harness, '3', '4');
        let current = commit_clean_pair(&harness, '1', '2');
        let (code, out, err) = harness.run(&["clean", "--apply", "--prune-unobserved"]);
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
        let (code, out, err) = harness.run(&["clean", "--apply", "--prune-unobserved"]);
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
        let (code, out, err) = harness.run(&["clean", "--apply", "--prune-unobserved"]);
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
        let (code, out, err) =
            harness.run(&["clean", "--apply", "--prune-unobserved", "--output=json"]);
        assert_eq!(code, 0, "{out}{err}");
        let events = json_events(&out);
        let pruned = events
            .iter()
            .find(|event| {
                event["event"] == serde_json::json!("notice")
                    && event["code"] == serde_json::json!("clean_pruned")
            })
            .expect("clean_pruned notice");
        let message = pruned["message"].as_str().expect("message");
        assert!(message.starts_with("pruned "), "{message}");
        assert!(message.ends_with(" bytes)"), "{message}");
        assert_eq!(pruned["related_command"], serde_json::json!("clean"));
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
        let (code, out, err) = harness.run(&["clean", "--prune-unobserved"]);
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
        let (default_code, default_out, _) = default.run(&["clean", "--prune-unobserved"]);
        let explicit = Harness::new("clean-check-matches-explicit");
        let estale = commit_clean_pair(&explicit, '3', '4');
        let _ecurrent = commit_clean_pair(&explicit, '1', '2');
        assert_eq!(stale, estale, "fixtures agree");
        let (explicit_code, explicit_out, _) =
            explicit.run(&["clean", "--check", "--prune-unobserved"]);
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
        let (code, out, err) = harness.run(&["clean", "--prune-unobserved", "--output=json"]);
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
        assert!(message.starts_with("would prune "), "{message}");
        assert!(message.ends_with(" bytes)"), "{message}");
        assert_eq!(notice["related_command"], serde_json::json!("clean"));
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

    fn preserved_unobserved_plan() -> dx_clean::CleanPlan {
        dx_clean::CleanPlan {
            prune_setup_records: Vec::new(),
            prune_generations: Vec::new(),
            refused_unmanaged: Vec::new(),
            preserved_current: Some("1".repeat(64)),
            observation_unknown: true,
            preserved_unobserved_setup_records: vec!["3".repeat(64)],
            preserved_unobserved_generations: vec![dx_clean::GenerationView {
                kind: lease::GenerationUse::Environment,
                hex: "3".repeat(64),
            }],
        }
    }

    fn preserved_unobserved_bytes() -> dx_clean::PruneBytes {
        dx_clean::PruneBytes {
            setup_record_bytes: vec![("3".repeat(64), 11)],
            generation_bytes: vec![(
                dx_clean::GenerationView {
                    kind: lease::GenerationUse::Environment,
                    hex: "3".repeat(64),
                },
                22,
            )],
        }
    }

    fn run_check(json: bool, verbose: bool) -> (i32, String) {
        let mut out = Vec::new();
        let code = super::check_clean(
            &mut out,
            &preserved_unobserved_plan(),
            &dx_clean::PruneBytes::default(),
            &preserved_unobserved_bytes(),
            json,
            verbose,
        );
        (code, String::from_utf8(out).expect("utf8 output"))
    }

    fn run_report(json: bool) -> String {
        let mut out = Vec::new();
        super::report_applied(
            &mut out,
            &preserved_unobserved_plan(),
            &dx_clean::CleanOutcome::default(),
            &dx_clean::PruneBytes::default(),
            &preserved_unobserved_bytes(),
            json,
            true,
        )
        .expect("report");
        String::from_utf8(out).expect("utf8 output")
    }

    #[test]
    fn clean_check_preserved_unobserved_lists_kept_state() {
        let hex = "3".repeat(64);
        let (code, out) = run_check(false, true);
        assert_eq!(code, 0, "{out}");
        assert!(
            out.contains(&format!(
                "preserve unobserved setup record: .dx/setups/{hex}"
            )),
            "{out}"
        );
        assert!(
            out.contains(&format!(
                "preserve unobserved generation: .dx/environments/{hex}"
            )),
            "{out}"
        );
        assert!(out.contains("process observation unavailable"), "{out}");
        assert!(
            out.contains("run `dx clean --apply --prune-unobserved` to prune it anyway"),
            "{out}"
        );
        assert!(!out.contains("nothing to prune"), "{out}");
        assert!(!out.contains("run `dx clean --apply` to prune it"), "{out}");
    }

    #[test]
    fn clean_check_preserved_unobserved_json_notices() {
        let hex = "3".repeat(64);
        let (code, out) = run_check(true, true);
        assert_eq!(code, 0, "{out}");
        let events = json_events(&out);
        let preserved: Vec<&serde_json::Value> = events
            .iter()
            .filter(|event| event["code"] == serde_json::json!("clean_preserved_unobserved"))
            .collect();
        assert_eq!(preserved.len(), 2, "{out}");
        assert_eq!(
            preserved[0]["path"],
            serde_json::json!(format!(".dx/setups/{hex}"))
        );
        assert_eq!(
            preserved[1]["path"],
            serde_json::json!(format!(".dx/environments/{hex}"))
        );
        assert_eq!(
            preserved[0]["message"],
            serde_json::json!(format!(
                "preserved unobserved setup record .dx/setups/{hex} (observation unavailable, 11 bytes)"
            ))
        );
        assert_eq!(
            preserved[1]["message"],
            serde_json::json!(format!(
                "preserved unobserved generation .dx/environments/{hex} (observation unavailable, 22 bytes)"
            ))
        );
        assert!(!out.contains("clean_planned"), "{out}");
        let last = events.last().expect("finished");
        assert_eq!(last["event"], serde_json::json!("command_finished"));
        assert_eq!(last["exit_code"], serde_json::json!(0));
    }

    #[test]
    fn clean_apply_json_leased_notice_carries_size() {
        let leased = dx_clean::GenerationView {
            kind: lease::GenerationUse::Generated,
            hex: "4".repeat(64),
        };
        let outcome = dx_clean::CleanOutcome {
            removed_setup_records: Vec::new(),
            removed_generations: Vec::new(),
            skipped_leased_generations: vec![leased.clone()],
        };
        let bytes = dx_clean::PruneBytes {
            setup_record_bytes: Vec::new(),
            generation_bytes: vec![(leased, 33)],
        };
        let mut out = Vec::new();
        super::emit_clean_pruned(&mut out, &outcome, &bytes).expect("emit");
        let out = String::from_utf8(out).expect("utf8 output");
        let events = json_events(&out);
        assert_eq!(events.len(), 1, "{out}");
        assert_eq!(
            events[0]["code"],
            serde_json::json!("clean_preserved_leased")
        );
        assert_eq!(
            events[0]["message"],
            serde_json::json!(format!(
                "preserved leased generation .dx/generated/{} (a live reader holds it, 33 bytes)",
                "4".repeat(64)
            ))
        );
    }

    #[test]
    fn clean_check_preserved_unobserved_quiet_is_silent() {
        let (code, out) = run_check(false, false);
        assert_eq!(code, 0, "{out}");
        assert_eq!(out, "");
    }

    #[test]
    fn clean_apply_reports_preserved_unobserved() {
        let hex = "3".repeat(64);
        let text = run_report(false);
        assert!(
            text.contains("dx clean: preserved 1 unobserved setup records and 1 unobserved generations (process observation unavailable)"),
            "{text}"
        );
        assert!(
            text.contains("run `dx clean --apply --prune-unobserved` to prune it anyway"),
            "{text}"
        );
        assert!(!text.contains("nothing to prune"), "{text}");
        let json = run_report(true);
        let events = json_events(&json);
        let paths: Vec<&serde_json::Value> = events
            .iter()
            .filter(|event| event["code"] == serde_json::json!("clean_preserved_unobserved"))
            .map(|event| &event["path"])
            .collect();
        assert_eq!(
            paths,
            vec![
                &serde_json::json!(format!(".dx/setups/{hex}")),
                &serde_json::json!(format!(".dx/environments/{hex}")),
            ],
            "{json}"
        );
        assert!(!json.contains("nothing to prune"), "{json}");
    }
}
