use super::common::*;
use super::managed_prepare::{map_commit_error, prepare_managed_sides};
use dx_output::{
    ActionCounts, FinishedCounts, OutputMode, command_finished, command_started, operation_event,
    selection_event,
};
use dx_plan::args::Invocation;
use dx_plan::plan::{bep_path, plan_managed, plan_managed_with_roots};
use dx_plan::resolve::expand_codegen_roots;
use dx_process::ForwardError;

/// Prints the direnv lines. `.dx/local/bin` joins the path once it exists; the selected setup
/// is watched either way so direnv reloads after `dx sync`. Runs no Bazel.
pub(crate) fn execute_env_shell(env: Env<'_>) -> i32 {
    let Env { workspace, out, .. } = env;
    let local = dx_path::state::local_dir(workspace);
    if local.join(dx_path::state::BIN).is_dir() {
        let _ = writeln!(out, "PATH_add .dx/local/bin\nwatch_file .dx/local/bin");
    }
    let _ = writeln!(out, "watch_file .dx/local/{}", dx_path::state::CURRENT);
    0
}

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
        actions,
        proc_root,
        ..
    } = env;
    if !invocation.command.is_managed() {
        return pre_exec(
            invocation.output,
            out,
            err,
            &ForwardError::UnsupportedCommand {
                command: invocation.command.name().to_owned(),
            }
            .to_string(),
        );
    }
    let scope = match dx_setup::resolve_scope(&invocation.targets) {
        Ok(scope) => scope,
        Err(error) => return pre_exec(invocation.output, out, err, &error.to_string()),
    };
    let bep = bep_path(temp_dir, pid, nonce);
    let Some(bep_text) = bep.to_str() else {
        return operational(
            invocation,
            out,
            err,
            CODE_UNREADABLE_BEP,
            &crate::startup::temp_path_not_utf8(),
        );
    };
    let expanded: Option<Vec<String>> = match &scope {
        dx_setup::SetupScope::Exact(label) => match expand_codegen_roots(
            label,
            workspace,
            query_runner,
            &invocation.bazel_startup_options,
        ) {
            Ok(roots) => Some(roots),
            Err(error) => return resolve_failed(invocation.output, out, err, &error),
        },
        dx_setup::SetupScope::Repository => None,
    };
    let bazel_options = invocation.quiet_bazel_options();
    let plan = match expanded {
        Some(ref roots) => plan_managed_with_roots(
            invocation.command,
            roots,
            &bazel_options,
            bep_text,
            &invocation.bazel_startup_options,
        ),
        None => plan_managed(
            invocation.command,
            &scope,
            &bazel_options,
            bep_text,
            &invocation.bazel_startup_options,
        ),
    };
    let plan = match plan {
        Ok(plan) => plan,
        Err(error) => return pre_exec(invocation.output, out, err, &format!("{error}")),
    };
    let verbose = invocation.chatty();
    let json = invocation.output == OutputMode::Json;
    let op_scope: Option<Vec<String>> = match (&scope, &expanded) {
        (_, Some(roots)) => Some(roots.clone()),
        (dx_setup::SetupScope::Exact(label), None) => Some(vec![label.clone()]),
        (dx_setup::SetupScope::Repository, None) => None,
    };
    if json {
        if let Err(exit) = emit_built(out, command_started(invocation.command.name(), "default")) {
            return exit;
        }
        if let Err(exit) = emit_built(
            out,
            operation_event(invocation.command.name(), "collect", op_scope.as_deref()),
        ) {
            return exit;
        }
    }
    if !json
        && verbose
        && let Err(exit) = check_stdout_write(writeln!(out, "{}", plan.summary))
    {
        return exit;
    }
    let bazel_code = match run_bazel(invocation, out, err, workspace, runner, &plan.argv, &[]) {
        Ok(code) => code,
        Err(exit) => {
            let _ = std::fs::remove_file(&bep);
            return exit;
        }
    };
    let action_counts = read_actions(&bep, actions);
    if bazel_code != 0 {
        let _ = std::fs::remove_file(&bep);
        if json {
            let scope_text = op_scope
                .as_deref()
                .map(|scope| scope.join(" "))
                .unwrap_or_else(|| "//...".to_owned());
            if let Err(exit) = emit_built(
                out,
                bazel_exit_event(
                    bazel_failure_code(bazel_code),
                    &format!(
                        "Bazel collection build failed with exit {bazel_code} for {scope_text} (see stderr diagnostics)"
                    ),
                    bazel_code,
                    "collect",
                ),
            ) {
                return kept_child(err, bazel_code, exit);
            }
            let finished = command_finished(bazel_code, &with_actions(action_counts));
            if let Err(exit) = emit_event(out, &finished) {
                return kept_child(err, bazel_code, exit);
            }
        }
        return bazel_code;
    }
    let (sides, staged_leases) = match prepare_managed_sides(workspace, workspace, &bep) {
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
    drop(staged_leases);
    let _ = std::fs::remove_file(&bep);
    super::sync_prune::prune_and_warn(workspace, proc_root, invocation.output, out, err);
    super::ide_refresh::refresh_and_warn(
        workspace,
        query_runner,
        &invocation.bazel_startup_options,
        invocation.output,
        out,
        err,
    );
    if json {
        let setup_id = dx_setup::setup_hex(&pair);
        if let Err(exit) = emit_built(
            out,
            selection_event(
                &setup_id,
                pair.environment.as_str(),
                pair.generated.as_str(),
            ),
        ) {
            return exit;
        }
        if let Err(exit) = emit_event(out, &command_finished(0, &with_actions(action_counts))) {
            return exit;
        }
        return 0;
    }
    if verbose {
        let setup = dx_setup::setup_hex(&pair);
        if outcome == dx_setup::CommitOutcome::AlreadyCurrent {
            let written = writeln!(
                out,
                "dx {}: already selected setup {setup} (environment {}, generated {})",
                invocation.command.name(),
                pair.environment.as_str(),
                pair.generated.as_str(),
            );
            if let Err(exit) = check_stdout_write(written) {
                return exit;
            }
        } else {
            let written = writeln!(
                out,
                "dx {}: selected setup {setup} (environment {}, generated {})",
                invocation.command.name(),
                pair.environment.as_str(),
                pair.generated.as_str(),
            );
            if let Err(exit) = check_stdout_write(written) {
                return exit;
            }
        }
    }
    0
}

fn with_actions(actions: Option<ActionCounts>) -> FinishedCounts {
    FinishedCounts {
        actions,
        ..FinishedCounts::default()
    }
}

#[cfg(test)]
mod tests {
    use super::super::managed_codegen::{empty_generated_id, stage_codegen_side};
    use super::super::managed_env::{empty_env_id, stage_env_side};
    use super::super::test_support::*;
    use dx_setup::{ENVIRONMENTS_DIR_NAME, GENERATED_DIR_NAME, read_current_pair};

    #[test]
    fn env_shell_succeeds_before_any_bin_exists() {
        let harness = Harness::new("env-shell-no-bin");
        let (code, out, err) = harness.run(&["sync", "--shell=direnv"]);
        assert_eq!(code, 0, "{out}{err}");
        assert_eq!(out, "watch_file .dx/local/current\n");
        std::fs::create_dir_all(harness.workspace.join(".dx/local/bin")).expect("bin");
        let (code, out, err) = harness.run(&["sync", "--shell=direnv"]);
        assert_eq!(code, 0, "{out}{err}");
        assert!(out.starts_with("PATH_add .dx/local/bin\n"), "{out}");
    }

    #[test]
    fn managed_live_empty_selection_commits_with_empty_counterparts() {
        {
            let command = "sync";
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
            for side in [ENVIRONMENTS_DIR_NAME, GENERATED_DIR_NAME] {
                let id = match side {
                    GENERATED_DIR_NAME => pair.generated.as_str(),
                    _ => pair.environment.as_str(),
                };
                assert!(
                    dx_path::state::local_dir(&harness.workspace)
                        .join(side)
                        .join(id)
                        .is_dir(),
                    "{command} stages its {side} generation"
                );
            }
            let (code, out, err) = harness.run(&[command]);
            assert_eq!(code, 0, "{out}{err}");
            assert!(out.contains("already selected setup "), "{out}");
            assert_eq!(err, "", "{err}");
        }
    }

    fn quiet_proc(harness: &Harness) -> std::path::PathBuf {
        let proc_root = harness.workspace.join("quiet-proc");
        std::fs::create_dir_all(&proc_root).expect("empty process table");
        proc_root
    }

    #[test]
    fn sync_prunes_the_superseded_setup() {
        let mut harness = Harness::new("sync-prunes-superseded");
        harness.proc_root = quiet_proc(&harness);
        let old = commit_clean_pair(&harness, '1', '2');
        let (code, out, err) = harness.run(&["sync"]);
        assert_eq!(code, 0, "{out}{err}");
        assert_eq!(err, "", "{err}");
        let dx_dir = dx_path::state::local_dir(&harness.workspace);
        assert!(!dx_dir.join("setups").join(&old).exists(), "{out}");
        assert!(
            !dx_dir
                .join(ENVIRONMENTS_DIR_NAME)
                .join("1".repeat(64))
                .exists()
        );
        assert!(
            !dx_dir
                .join(GENERATED_DIR_NAME)
                .join("2".repeat(64))
                .exists()
        );
        let pair = read_current_pair(&harness.workspace)
            .expect("read current")
            .expect("selection committed");
        let current = dx_setup::setup_hex(&pair);
        assert!(dx_dir.join("setups").join(current).exists());
    }

    #[test]
    fn sync_keeps_a_leased_generation() {
        let mut harness = Harness::new("sync-keeps-leased");
        harness.proc_root = quiet_proc(&harness);
        commit_clean_pair(&harness, '1', '2');
        let dx_dir = dx_path::state::local_dir(&harness.workspace);
        let leased_hex = "2".repeat(64);
        let _lease = dx_atomic_fs::lease::acquire_shared(
            &dx_dir,
            dx_atomic_fs::lease::GenerationUse::Generated,
            &leased_hex,
            std::time::Duration::from_secs(10),
        )
        .expect("hold a reader lease");
        let (code, out, err) = harness.run(&["sync"]);
        assert_eq!(code, 0, "{out}{err}");
        assert!(dx_dir.join(GENERATED_DIR_NAME).join(&leased_hex).exists());
        assert!(
            !dx_dir
                .join(ENVIRONMENTS_DIR_NAME)
                .join("1".repeat(64))
                .exists()
        );
    }

    #[test]
    fn sync_keeps_old_setups_when_observation_is_unavailable() {
        let mut harness = Harness::new("sync-unobserved");
        harness.proc_root = harness.workspace.join("no-such-proc");
        let old = commit_clean_pair(&harness, '1', '2');
        let (code, out, err) = harness.run(&["sync"]);
        assert_eq!(code, 0, "{out}{err}");
        let dx_dir = dx_path::state::local_dir(&harness.workspace);
        assert!(dx_dir.join("setups").join(old).exists());
    }

    #[test]
    fn managed_live_exact_sides_commit_with_empty_counterparts() {
        {
            let command = "sync";
            let name = format!("managed-exact-{command}");
            let harness = Harness::new(&name);
            harness.query.script_stdout("\n");
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

    fn prost_fixture_graph() -> Vec<String> {
        use dx_testing::cli::graph_rule_of_kind;
        vec![
            graph_rule_of_kind(
                "//generation:result_proto",
                "proto_library",
                &["//generation:result.proto"],
                &[],
            ),
            graph_rule_of_kind(
                "//generation:result_proto_rs",
                "rust_prost_library",
                &["//generation:result_proto"],
                &[],
            ),
            graph_rule_of_kind(
                "//generation:codegen_prost_fixture",
                "prost_codegen_shard",
                &["//generation:result_proto_rs"],
                &[],
            ),
        ]
    }

    #[test]
    fn managed_exact_env_expands_bare_schema_to_projections() {
        let harness = Harness::new("managed-expand-live");
        harness.query.script_graph(&prost_fixture_graph());
        let (code, out, err) = harness.run(&["sync", "//generation:result_proto"]);
        assert_eq!(code, 0, "{out}{err}");
        assert!(
            out.contains(
                "Running sync for //generation:codegen_prost_fixture //generation:result_proto"
            ),
            "{out}"
        );
        assert!(out.contains("selected setup "), "{out}");
    }

    #[test]
    fn managed_exact_env_expansion_failure_is_pre_exec() {
        use dx_plan::resolve::QueryResult;

        let harness = Harness::new("managed-expand-fail");
        harness.query.outputs.borrow_mut().push(QueryResult {
            code: Some(2),
            stdout: Vec::new(),
            stderr: b"query failed: blah".to_vec(),
        });
        let (code, _, err) = harness.run(&["sync", "//generation:result_proto"]);
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
        let (code, out, err) = harness.run(&["sync", "--quiet"]);
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
        let (code, _, _) = harness.run(&["sync"]);
        assert_eq!(code, 0);
        let generations = dx_path::state::local_dir(&harness.workspace).join(GENERATED_DIR_NAME);
        assert!(generations.is_dir(), "first commit stages generations");
        let pointer = harness.workspace.join(".dx/local/setups/current");
        dx_test_scratch::remove_directory_link(&pointer).expect("remove pointer");
        std::fs::write(&pointer, "not a symlink").expect("file pointer");
        let (code, _, err) = harness.run(&["sync"]);
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
        let (staged, _lease) =
            stage_codegen_side(workspace, workspace, &codegen_plan).expect("stage");
        assert_eq!(staged, empty_generated_id());
        let env_plan = dx_env_plan::collect_plan(&[]).expect("empty env plan");
        let (staged, _held) = stage_env_side(workspace, &env_plan).expect("stage");
        assert_eq!(staged, empty_env_id());
        let values = std::fs::read_to_string(
            dx_path::state::local_dir(&workspace)
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
        let (code, _, err) = harness.run(&["sync"]);
        assert_eq!(code, 1);
        assert!(err.contains("dx: launch_failed: failed to launch Bazel"));
    }

    #[test]
    fn managed_live_signalled_bazel_is_operational() {
        let harness = Harness {
            signalled: true,
            ..Harness::new("managed-signalled")
        };
        let (code, _, err) = harness.run(&["sync"]);
        assert_eq!(code, 1);
        assert!(err.contains("dx: bazel_signalled: Bazel terminated by signal"));
    }

    #[test]
    fn managed_live_bazel_failure_returns_exit_verbatim() {
        let harness = Harness {
            bazel_code: 3,
            ..Harness::new("managed-bazel-failed")
        };
        let (code, out, err) = harness.run(&["sync"]);
        assert_eq!(code, 3, "{out}{err}");
        assert!(out.contains("Running sync for //..."), "{out}");
    }

    #[test]
    fn managed_live_missing_bep_is_operational() {
        let harness = Harness {
            skip_bep: true,
            ..Harness::new("managed-missing-bep")
        };
        let (code, _, err) = harness.run(&["sync"]);
        assert_eq!(code, 1);
        assert!(err.contains("dx: unreadable_bep: failed to read build events"));
    }

    #[test]
    fn managed_live_malformed_bep_is_operational() {
        let harness = Harness {
            raw_bep: Some(vec!["{not json".to_owned()]),
            ..Harness::new("managed-bad-bep")
        };
        let (code, _, err) = harness.run(&["sync"]);
        assert_eq!(code, 1);
        assert!(err.contains("dx: invalid_bep: invalid build events"));
    }

    #[test]
    fn managed_policy_conflict_fails_before_execution() {
        let harness = Harness::new("managed-conflict");
        let (code, _, _) = harness.run(&["sync", "--", "--aspects=//other.bzl%aspect"]);
        assert_eq!(code, 2);
        assert!(
            harness.seen_env.borrow().is_empty(),
            "policy conflict launches nothing"
        );
    }

    #[test]
    fn managed_live_json_streams_selection() {
        {
            let command = "sync";
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
                assert!(
                    value
                        .chars()
                        .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
                );
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
        let (code, out, _) = harness.run(&["sync", "--output=json"]);
        assert_eq!(code, 3, "{out}");
        let events = json_events(&out);
        let error = event(&events, "error");
        assert_eq!(error["code"], serde_json::json!("bazel_failed"), "{out}");
        assert_eq!(error["cause"], serde_json::json!("bazel"), "{out}");
        assert_eq!(error["status"], serde_json::json!(3), "{out}");
        assert_eq!(error["phase"], serde_json::json!("collect"), "{out}");
        assert!(out.contains("command_finished"), "{out}");
        assert!(out.contains("\"exit_code\":3"), "{out}");
    }

    #[test]
    fn managed_check_current_selection_succeeds_without_touching_state() {
        {
            let command = "sync";
            let name = format!("managed-check-current-{command}");
            let harness = Harness::new(&name);
            let (code, _, err) = harness.run(&[command]);
            assert_eq!(code, 0, "{err}");
            let before = read_current_pair(&harness.workspace)
                .expect("read current")
                .expect("selection committed");
            let (code, out, err) = harness.run(&[command]);
            assert_eq!(code, 0, "{out}{err}");
            assert!(out.contains("already selected setup "), "{out}");
            assert_eq!(err, "", "{err}");
            assert_eq!(
                read_current_pair(&harness.workspace)
                    .expect("reread current")
                    .expect("selection kept"),
                before,
                "{command} check keeps the committed selection"
            );
        }
    }

    #[test]
    fn sync_json_reports_action_counts() {
        let mut harness = Harness::new("managed-actions");
        let (_, out, _) = harness.run(&["sync", "--output=json"]);
        assert_eq!(finished_actions(&out), None, "{out}");
        harness.build_metrics = Some(build_metrics_line(10, 9, 2, 3, 5));
        let (code, out, err) = harness.run(&["sync", "--output=json"]);
        assert_eq!(code, 0, "{out}{err}");
        assert_eq!(
            finished_actions(&out),
            Some(
                serde_json::json!({"created": 10, "executed": 4, "cache_hits": {"disk": 2, "remote": 3, "local": 5}})
            )
        );
    }
}

#[cfg(test)]
#[path = "managed_delivery_tests.rs"]
mod managed_delivery_tests;
