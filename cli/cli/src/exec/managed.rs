use super::common::*;
use super::managed_prepare::{map_commit_error, prepare_managed_sides};
use crate::args::{Command, Invocation};
use crate::plan::{bep_path, plan_managed, plan_managed_with_roots};
use crate::resolve::expand_codegen_roots;
use dx_output::{
    command_finished, command_started, error_event, operation_event, selection_event, write_event,
    FinishedCounts, OutputMode,
};
use dx_process::ForwardError;

pub(crate) fn execute_managed(invocation: &Invocation, env: Env<'_>) -> i32 {
    debug_assert!(
        invocation.command.is_managed(),
        "managed dispatch guards commands"
    );
    let Env {
        workspace,
        runner,
        query_runner,
        temp_dir,
        pid,
        nonce,
        out,
        err,
        ..
    } = env;
    if !invocation.command.is_managed() {
        return pre_exec(
            err,
            &ForwardError::UnsupportedCommand {
                command: invocation.command.name().to_owned(),
            }
            .to_string(),
        );
    }
    let scope = match dx_setup::resolve_scope(&invocation.targets) {
        Ok(scope) => scope,
        Err(error) => return pre_exec(err, &error.to_string()),
    };
    let bep = bep_path(temp_dir, pid, nonce);
    let Some(bep_text) = bep.to_str() else {
        return operational(
            invocation,
            out,
            err,
            CODE_UNREADABLE_BEP,
            "temporary event path is not UTF-8",
        );
    };
    let expanded: Option<Vec<String>> = match (invocation.command, &scope) {
        (Command::Codegen | Command::Setup, dx_setup::SetupScope::Exact(label)) => {
            match expand_codegen_roots(label, workspace, query_runner) {
                Ok(roots) => Some(roots),
                Err(error) => return pre_exec(err, &error.to_string()),
            }
        }
        _ => None,
    };
    let plan = match expanded {
        Some(ref roots) => plan_managed_with_roots(
            invocation.command,
            roots,
            &invocation.bazel_options,
            bep_text,
        ),
        None => plan_managed(
            invocation.command,
            &scope,
            &invocation.bazel_options,
            bep_text,
        ),
    };
    let plan = match plan {
        Ok(plan) => plan,
        Err(error) => return pre_exec(err, &format!("{error}")),
    };
    let verbose = invocation.chatty();
    let json = invocation.output == OutputMode::Json;
    let op_scope: Option<Vec<String>> = match (&scope, &expanded) {
        (_, Some(roots)) => Some(roots.clone()),
        (dx_setup::SetupScope::Exact(label), None) => Some(vec![label.clone()]),
        (dx_setup::SetupScope::Repository, None) => None,
    };
    if json {
        if let Ok(event) = command_started(invocation.command.name(), invocation.dry_run, "default")
        {
            let _ = write_event(out, &event);
        }
        if let Ok(event) =
            operation_event(invocation.command.name(), "collect", op_scope.as_deref())
        {
            let _ = write_event(out, &event);
        }
    }
    if invocation.dry_run {
        if json {
            let _ = write_event(out, &command_finished(0, &FinishedCounts::default()));
        } else if verbose {
            let _ = writeln!(out, "{}", plan.summary);
        }
        return 0;
    }
    if !json && verbose {
        let _ = writeln!(out, "{}", plan.summary);
    }
    let bazel_code = match run_bazel(invocation, out, err, workspace, runner, &plan.argv, &[]) {
        Ok(code) => code,
        Err(exit) => {
            let _ = std::fs::remove_file(&bep);
            return exit;
        }
    };
    if bazel_code != 0 {
        let _ = std::fs::remove_file(&bep);
        if json {
            let scope_text = op_scope
                .as_deref()
                .map(|scope| scope.join(" "))
                .unwrap_or_else(|| "//...".to_owned());
            if let Ok(event) = error_event(
                "bazel_failed",
                &format!(
                    "Bazel collection build failed with exit {bazel_code} for {scope_text} (see stderr diagnostics)"
                ),
                None,
                None,
                Some("collect"),
            ) {
                let _ = write_event(out, &event);
            }
            let _ = write_event(
                out,
                &command_finished(bazel_code, &FinishedCounts::default()),
            );
        }
        return bazel_code;
    }
    let repository = matches!(scope, dx_setup::SetupScope::Repository);
    let (sides, _leases) =
        match prepare_managed_sides(invocation.command, repository, workspace, &bep) {
            Ok(prepared) => prepared,
            Err((code, message)) => {
                let _ = std::fs::remove_file(&bep);
                return operational(invocation, out, err, &code, &message);
            }
        };
    let (pair, outcome) = match dx_setup::commit_prepared(workspace, sides) {
        Ok(committed) => committed,
        Err(error) => {
            let (code, message) = map_commit_error(error);
            let _ = std::fs::remove_file(&bep);
            return operational(invocation, out, err, &code, &message);
        }
    };
    let _ = std::fs::remove_file(&bep);
    if json {
        let setup_id = dx_setup::setup_hex(&pair);
        if let Ok(event) = selection_event(
            &setup_id,
            pair.environment.as_str(),
            pair.generated.as_str(),
        ) {
            let _ = write_event(out, &event);
        }
        let _ = write_event(out, &command_finished(0, &FinishedCounts::default()));
        return 0;
    }
    if verbose {
        let setup = dx_setup::setup_hex(&pair);
        if outcome == dx_setup::CommitOutcome::AlreadyCurrent {
            let _ = writeln!(
                out,
                "dx {}: already selected setup {setup} (environment {}, generated {})",
                invocation.command.name(),
                pair.environment.as_str(),
                pair.generated.as_str(),
            );
        } else {
            let _ = writeln!(
                out,
                "dx {}: selected setup {setup} (environment {}, generated {})",
                invocation.command.name(),
                pair.environment.as_str(),
                pair.generated.as_str(),
            );
        }
    }
    0
}

#[cfg(test)]
mod tests {
    use super::super::managed_codegen::{empty_generated_id, stage_codegen_side};
    use super::super::managed_env::{empty_env_id, stage_env_side};
    use super::super::test_support::*;
    use dx_setup::{read_current_pair, ENVIRONMENTS_DIR_NAME, GENERATED_DIR_NAME};

    #[test]
    fn managed_dry_run_prints_summary_without_launching() {
        for command in ["codegen", "env", "setup"] {
            let name = format!("managed-dryrun-{command}");
            let harness = Harness::new(&name);
            let (code, out, err) = harness.run(&[command, "--dry-run"]);
            assert_eq!(code, 0, "{out}{err}");
            assert!(
                out.contains(&format!("Running {command} for //...")),
                "{out}"
            );
            assert_eq!(err, "", "{err}");
            assert!(
                harness.seen_env.borrow().is_empty(),
                "dry-run launches nothing"
            );
        }
    }

    #[test]
    fn managed_dry_run_exact_scope_selects_label() {
        let harness = Harness::new("managed-dryrun-exact");
        let (code, out, err) = harness.run(&["env", "//a:one", "--dry-run"]);
        assert_eq!(code, 0, "{out}{err}");
        assert!(out.contains("Running env for //a:one"), "{out}");
        assert_eq!(err, "", "{err}");
        assert!(
            harness.seen_env.borrow().is_empty(),
            "dry-run launches nothing"
        );
    }

    #[test]
    fn managed_dry_run_quiet_prints_nothing() {
        let harness = Harness::new("managed-dryrun-quiet");
        let (code, out, err) = harness.run(&["setup", "--dry-run", "--quiet"]);
        assert_eq!(code, 0, "{out}{err}");
        assert_eq!(out, "", "{out}");
        assert_eq!(err, "", "{err}");
    }

    #[test]
    fn managed_live_empty_selection_commits_with_empty_counterparts() {
        for command in ["codegen", "env", "setup"] {
            let name = format!("managed-commit-{command}");
            let harness = Harness::new(&name);
            let (code, out, err) = harness.run(&[command]);
            assert_eq!(code, 0, "{out}{err}");
            assert!(
                out.contains(&format!("Running {command} for //...")),
                "{out}"
            );
            assert!(out.contains("selected setup "), "{out}");
            assert_eq!(err, "", "{err}");
            assert_eq!(
                harness.seen_env.borrow().len(),
                1,
                "committed selection launches one Bazel build"
            );
            let pair = read_current_pair(&harness.workspace)
                .expect("read current")
                .expect("selection committed");
            assert_eq!(pair.environment, empty_env_id());
            assert_eq!(pair.generated, empty_generated_id());
            for side in match command {
                "codegen" => vec![GENERATED_DIR_NAME],
                "env" => vec![ENVIRONMENTS_DIR_NAME],
                _ => vec![ENVIRONMENTS_DIR_NAME, GENERATED_DIR_NAME],
            } {
                let id = match side {
                    GENERATED_DIR_NAME => pair.generated.as_str(),
                    _ => pair.environment.as_str(),
                };
                assert!(
                    harness.workspace.join(".dx").join(side).join(id).is_dir(),
                    "{command} stages its {side} generation"
                );
                let kind = if side == GENERATED_DIR_NAME {
                    dx_clean::GenerationKind::Generated
                } else {
                    dx_clean::GenerationKind::Environment
                };
                assert!(
                    dx_clean::lease_path(&harness.workspace.join(".dx"), kind, id).is_file(),
                    "{command} leases its staged {side} generation across commit"
                );
                assert!(
                    dx_clean::try_exclusive_lease(&harness.workspace.join(".dx"), kind, id)
                        .expect("lease attempt")
                        .is_some(),
                    "{command} releases its {side} lease after commit"
                );
            }
            let (code, out, err) = harness.run(&[command]);
            assert_eq!(code, 0, "{out}{err}");
            assert!(out.contains("already selected setup "), "{out}");
            assert_eq!(err, "", "{err}");
        }
    }

    #[test]
    fn managed_live_exact_sides_commit_with_empty_counterparts() {
        for command in ["codegen", "env"] {
            let name = format!("managed-exact-{command}");
            let harness = Harness::new(&name);
            if command == "codegen" {
                harness.query.script_owners("\n");
            }
            let (code, out, err) = harness.run(&[command, "//a:one"]);
            assert_eq!(code, 0, "{out}{err}");
            assert!(out.contains("selected setup "), "{out}");
            let pair = read_current_pair(&harness.workspace)
                .expect("read current")
                .expect("selection committed");
            assert_eq!(pair.environment, empty_env_id());
            assert_eq!(pair.generated, empty_generated_id());
        }
    }

    #[test]
    fn managed_live_exact_setup_without_capability_fails_closed() {
        let harness = Harness::new("managed-setup-nocap");
        harness.query.script_owners("\n");
        let (code, out, err) = harness.run(&["setup", "//a:one"]);
        assert_eq!(code, 1, "{out}{err}");
        assert!(err.contains("dx: no_capability:"), "{err}");
        assert!(out.contains("Running setup for //a:one"), "{out}");
        assert_eq!(
            read_current_pair(&harness.workspace).expect("read current"),
            None,
            "capability failure commits nothing"
        );
    }

    #[test]
    fn managed_exact_codegen_expands_bare_schema_to_projections() {
        let harness = Harness::new("managed-expand-dryrun");
        harness
            .query
            .script_owners("//generation:codegen_prost_fixture\n");
        let (code, out, err) = harness.run(&["codegen", "//generation:result_proto", "--dry-run"]);
        assert_eq!(code, 0, "{out}{err}");
        assert!(
            out.contains(
                "Running codegen for //generation:codegen_prost_fixture //generation:result_proto"
            ),
            "{out}"
        );
        assert_eq!(err, "", "{err}");
        assert!(
            harness.seen_env.borrow().is_empty(),
            "dry-run launches no build"
        );
        assert_eq!(
            harness.query.calls.borrow().len(),
            1,
            "dry-run still runs the expansion query"
        );
        assert!(
            harness.query.calls.borrow()[0].last().expect("expression")
                == "kind('.*codegen_shard rule', rdeps(//..., set(\"//generation:result_proto\")))",
            "expansion queries shard rdeps: {:?}",
            harness.query.calls.borrow()[0]
        );

        let harness = Harness::new("managed-expand-live");
        harness
            .query
            .script_owners("//generation:codegen_prost_fixture\n");
        let (code, out, err) = harness.run(&["codegen", "//generation:result_proto"]);
        assert_eq!(code, 0, "{out}{err}");
        assert!(
            out.contains(
                "Running codegen for //generation:codegen_prost_fixture //generation:result_proto"
            ),
            "{out}"
        );
        assert!(out.contains("selected setup "), "{out}");

        let harness = Harness::new("managed-expand-setup");
        harness
            .query
            .script_owners("//generation:codegen_prost_fixture\n");
        let (code, out, err) = harness.run(&["setup", "//generation:result_proto", "--dry-run"]);
        assert_eq!(code, 0, "{out}{err}");
        assert!(
            out.contains(
                "Running setup for //generation:codegen_prost_fixture //generation:result_proto"
            ),
            "{out}"
        );
    }

    #[test]
    fn managed_exact_codegen_expansion_failure_is_pre_exec() {
        use crate::resolve::QueryResult;

        let harness = Harness::new("managed-expand-fail");
        harness.query.outputs.borrow_mut().push(QueryResult {
            code: Some(2),
            stdout: Vec::new(),
            stderr: b"query failed: blah".to_vec(),
        });
        let (code, _, err) = harness.run(&["codegen", "//generation:result_proto"]);
        assert_eq!(code, 2, "{err}");
        assert!(err.contains("query failed: blah"), "{err}");
        assert!(
            harness.seen_env.borrow().is_empty(),
            "expansion failure launches nothing"
        );
    }

    #[test]
    fn managed_live_quiet_commit_prints_nothing() {
        let harness = Harness::new("managed-quiet-commit");
        let (code, out, err) = harness.run(&["codegen", "--quiet"]);
        assert_eq!(code, 0, "{out}{err}");
        assert_eq!(out, "", "{out}");
        assert_eq!(err, "", "{err}");
        assert!(
            read_current_pair(&harness.workspace)
                .expect("read current")
                .is_some(),
            "quiet still commits"
        );
    }

    #[test]
    fn managed_live_malformed_current_fails_commit_without_mutation() {
        let harness = Harness::new("managed-bad-current");
        let (code, _, _) = harness.run(&["codegen"]);
        assert_eq!(code, 0);
        let generations = harness.workspace.join(".dx").join(GENERATED_DIR_NAME);
        assert!(generations.is_dir(), "first commit stages generations");
        let pointer = harness.workspace.join(".dx/setups/current");
        dx_test_scratch::remove_directory_link(&pointer).expect("remove pointer");
        std::fs::write(&pointer, "not a symlink").expect("file pointer");
        let (code, _, err) = harness.run(&["codegen"]);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("dx: managed_commit_failed:"), "{err}");
        assert!(
            generations.is_dir(),
            "commit failure preserves staged cache"
        );
        assert_eq!(
            std::fs::read(&pointer).expect("pointer bytes"),
            b"not a symlink",
            "commit failure leaves the malformed pointer untouched"
        );
    }

    #[test]
    fn managed_empty_sides_derive_the_managed_empty_identities() {
        let workspace = temp_dir("managed-empty-sides-ws");
        let workspace = workspace.path();
        let codegen_plan = dx_codegen::collect_plan(&[]).expect("empty codegen plan");
        let staged = stage_codegen_side(&workspace, &codegen_plan).expect("stage");
        assert_eq!(staged, empty_generated_id());
        let env_plan = dx_env_plan::collect_plan(&[]).expect("empty env plan");
        let staged = stage_env_side(&workspace, &env_plan).expect("stage");
        assert_eq!(staged, empty_env_id());
        let values = std::fs::read_to_string(
            workspace
                .join(".dx")
                .join(ENVIRONMENTS_DIR_NAME)
                .join(empty_env_id().as_str())
                .join("values.json"),
        )
        .expect("values");
        assert_eq!(values, "{}");
    }

    #[test]
    fn managed_live_launch_failure_is_operational() {
        let harness = Harness {
            io_error: true,
            ..Harness::new("managed-launch-failed")
        };
        let (code, _, err) = harness.run(&["codegen"]);
        assert_eq!(code, 1);
        assert!(err.contains("dx: launch_failed: failed to launch Bazel"));
    }

    #[test]
    fn managed_live_signalled_bazel_is_operational() {
        let harness = Harness {
            signalled: true,
            ..Harness::new("managed-signalled")
        };
        let (code, _, err) = harness.run(&["env"]);
        assert_eq!(code, 1);
        assert!(err.contains("dx: bazel_signalled: Bazel terminated by signal"));
    }

    #[test]
    fn managed_live_bazel_failure_returns_exit_verbatim() {
        let harness = Harness {
            bazel_code: 3,
            ..Harness::new("managed-bazel-failed")
        };
        let (code, out, err) = harness.run(&["setup"]);
        assert_eq!(code, 3, "{out}{err}");
        assert!(out.contains("Running setup for //..."), "{out}");
    }

    #[test]
    fn managed_live_missing_bep_is_operational() {
        let harness = Harness {
            skip_bep: true,
            ..Harness::new("managed-missing-bep")
        };
        let (code, _, err) = harness.run(&["codegen"]);
        assert_eq!(code, 1);
        assert!(err.contains("dx: unreadable_bep: failed to read build events"));
    }

    #[test]
    fn managed_live_malformed_bep_is_operational() {
        let harness = Harness {
            raw_bep: Some(vec!["{not json".to_owned()]),
            ..Harness::new("managed-bad-bep")
        };
        let (code, _, err) = harness.run(&["env"]);
        assert_eq!(code, 1);
        assert!(err.contains("dx: invalid_bep: invalid build events"));
    }

    #[test]
    fn managed_policy_conflict_fails_before_execution() {
        let harness = Harness::new("managed-conflict");
        let (code, _, _) = harness.run(&["setup", "--", "--aspects=//other.bzl%aspect"]);
        assert_eq!(code, 2);
        assert!(
            harness.seen_env.borrow().is_empty(),
            "policy conflict launches nothing"
        );
    }

    #[test]
    fn managed_dry_run_json_streams_planning_events() {
        for command in ["codegen", "env", "setup"] {
            let name = format!("managed-dry-json-{command}");
            let harness = Harness::new(&name);
            let (code, out, err) = harness.run(&[command, "--dry-run", "--output=json"]);
            assert_eq!(code, 0, "{out}{err}");
            let events = json_events(&out);
            let kinds = event_kinds(&events);
            assert_eq!(
                kinds,
                vec!["command_started", "operation", "command_finished"]
            );
            let op = event(&events, "operation");
            assert_eq!(op["command"], serde_json::json!(command));
            assert_eq!(op["phase"], serde_json::json!("collect"));
            assert!(op.get("scope").is_none(), "{op}");
            assert_eq!(
                events.last().expect("finished")["exit_code"],
                serde_json::json!(0)
            );
            assert_eq!(err, "", "{err}");
        }
    }

    #[test]
    fn managed_dry_run_json_exact_scope_includes_scope() {
        let harness = Harness::new("managed-dry-json-exact");
        let (code, out, err) = harness.run(&["env", "//a:one", "--dry-run", "--output=json"]);
        assert_eq!(code, 0, "{out}{err}");
        let events = json_events(&out);
        let op = event(&events, "operation");
        assert_eq!(op["scope"], serde_json::json!(["//a:one"]));
        assert_eq!(err, "", "{err}");
    }

    #[test]
    fn managed_live_json_streams_selection() {
        for command in ["codegen", "env", "setup"] {
            let name = format!("managed-live-json-{command}");
            let harness = Harness::new(&name);
            let (code, out, err) = harness.run(&[command, "--output=json"]);
            assert_eq!(code, 0, "{out}{err}");
            let events = json_events(&out);
            let kinds = event_kinds(&events);
            assert_eq!(
                kinds,
                vec![
                    "command_started",
                    "operation",
                    "selection",
                    "command_finished"
                ]
            );
            let selection = event(&events, "selection");
            for field in ["setup_id", "environment_id", "codegen_id"] {
                let value = selection[field].as_str().expect("hex");
                assert_eq!(value.len(), 64, "{selection}");
                assert!(value
                    .chars()
                    .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
            }
            assert_eq!(
                events.last().expect("finished")["exit_code"],
                serde_json::json!(0)
            );
            assert_eq!(err, "", "{err}");
        }
    }

    #[test]
    fn managed_live_json_bazel_failure_emits_explainer() {
        let harness = Harness {
            bazel_code: 3,
            ..Harness::new("managed-json-bazel-fail")
        };
        let (code, out, _) = harness.run(&["setup", "--output=json"]);
        assert_eq!(code, 3, "{out}");
        assert!(out.contains("bazel_failed"), "{out}");
        assert!(out.contains("command_finished"), "{out}");
        assert!(out.contains("\"exit_code\":3"), "{out}");
    }
}
