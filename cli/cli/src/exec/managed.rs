use super::common::*;
use super::managed_prepare::{map_commit_error, prepare_managed_sides};
use crate::args::{Command, Invocation};
use crate::plan::{bep_path, plan_managed, plan_managed_with_roots};
use crate::resolve::expand_codegen_roots;
use dx_output::{
    command_finished, command_started, notice_event, operation_event, selection_event,
    FinishedCounts, NoticeEvent, OutputMode,
};
use dx_process::ForwardError;
use std::path::Path;

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
            "temporary event path is not UTF-8",
        );
    };
    let expanded: Option<Vec<String>> = match (invocation.command, &scope) {
        (Command::Codegen, dx_setup::SetupScope::Exact(label)) => {
            match expand_codegen_roots(
                label,
                workspace,
                query_runner,
                &invocation.bazel_startup_options,
            ) {
                Ok(roots) => Some(roots),
                Err(error) => return resolve_failed(invocation.output, out, err, &error),
            }
        }
        _ => None,
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
    let apply = invocation.applies();
    let mode = if apply { "default" } else { "check" };
    let op_scope: Option<Vec<String>> = match (&scope, &expanded) {
        (_, Some(roots)) => Some(roots.clone()),
        (dx_setup::SetupScope::Exact(label), None) => Some(vec![label.clone()]),
        (dx_setup::SetupScope::Repository, None) => None,
    };
    if json {
        if let Err(exit) = emit_built(out, command_started(invocation.command.name(), mode)) {
            return exit;
        }
        if let Err(exit) = emit_built(
            out,
            operation_event(invocation.command.name(), "collect", op_scope.as_deref()),
        ) {
            return exit;
        }
    }
    if !json && verbose {
        if let Err(exit) = check_stdout_write(writeln!(out, "{}", plan.summary)) {
            return exit;
        }
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
            if let Err(exit) = emit_built(out, bazel_exit_event(
                CODE_BAZEL_FAILED,
                &format!(
                    "Bazel collection build failed with exit {bazel_code} for {scope_text} (see stderr diagnostics)"
                ),
                bazel_code,
                "collect",
            )) {
 return kept_child(err, bazel_code, exit);
 }
            let finished = command_finished(bazel_code, &FinishedCounts::default());
            if let Err(exit) = emit_event(out, &finished) {
                return kept_child(err, bazel_code, exit);
            }
        }
        return bazel_code;
    }
    if !apply {
        return check_managed(
            CheckInputs {
                invocation,
                workspace,
                temp_dir,
                scope: &scope,
                bep: &bep,
                json,
                verbose,
            },
            out,
            err,
        );
    }
    let (sides, staged_leases) =
        match prepare_managed_sides(invocation.command, workspace, workspace, &bep) {
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
        if let Err(exit) = emit_event(out, &command_finished(0, &FinishedCounts::default())) {
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

fn apply_hint(command: Command, scope: &dx_setup::SetupScope) -> String {
    match scope {
        dx_setup::SetupScope::Repository => format!("dx {} --apply", command.name()),
        dx_setup::SetupScope::Exact(label) => format!("dx {} --apply {label}", command.name()),
    }
}

struct CheckInputs<'a> {
    invocation: &'a Invocation,
    workspace: &'a Path,
    temp_dir: &'a Path,
    scope: &'a dx_setup::SetupScope,
    bep: &'a Path,
    json: bool,
    verbose: bool,
}

fn check_managed(
    inputs: CheckInputs<'_>,
    out: &mut dyn std::io::Write,
    err: &mut dyn std::io::Write,
) -> i32 {
    let CheckInputs {
        invocation,
        workspace,
        temp_dir,
        scope,
        bep,
        json,
        verbose,
    } = inputs;
    let scratch = temp_dir.join("managed-check");
    if let Err(error) = std::fs::create_dir_all(&scratch) {
        let _ = std::fs::remove_file(bep);
        return operational(
            invocation,
            out,
            err,
            CODE_MANAGED_COMMIT_FAILED,
            &format!("cannot create {}: {error}", scratch.display()),
        );
    }
    let (sides, staged_leases) =
        match prepare_managed_sides(invocation.command, workspace, &scratch, bep) {
            Ok(prepared) => prepared,
            Err((code, message)) => {
                let _ = std::fs::remove_file(bep);
                return operational(invocation, out, err, &code, &message);
            }
        };
    let current = match dx_setup::read_current_pair(workspace) {
        Ok(current) => current,
        Err(error) => {
            let (code, message) = map_commit_error(error);
            drop(staged_leases);
            let _ = std::fs::remove_file(bep);
            return operational(invocation, out, err, &code, &message);
        }
    };
    let intended = match dx_setup::resolve_pair(dx_setup::PairInputs {
        prepared_environment: sides.prepared_environment,
        prepared_generated: sides.prepared_generated,
        current: current.clone(),
        empty_environment: sides.empty_environment,
        empty_generated: sides.empty_generated,
    }) {
        Ok(intended) => intended,
        Err(_) => {
            let (code, message) = map_commit_error(dx_setup::CommitError::NoCapability);
            drop(staged_leases);
            let _ = std::fs::remove_file(bep);
            return operational(invocation, out, err, &code, &message);
        }
    };
    drop(staged_leases);
    let _ = std::fs::remove_file(bep);
    let setup = dx_setup::setup_hex(&intended);
    if current.as_ref() == Some(&intended) {
        if json {
            if let Err(exit) = emit_event(out, &command_finished(0, &FinishedCounts::default())) {
                return exit;
            }
            return 0;
        }
        if verbose {
            let written = writeln!(
                out,
                "dx {}: already selected setup {setup} (environment {}, generated {})",
                invocation.command.name(),
                intended.environment.as_str(),
                intended.generated.as_str(),
            );
            return match check_stdout_write(written) {
                Ok(()) => 0,
                Err(exit) => exit,
            };
        }
        return 0;
    }
    let detail = match &current {
        Some(pair) => format!(
            "stale selection: current setup {} want setup {setup} (environment {}, generated {})",
            dx_setup::setup_hex(pair),
            intended.environment.as_str(),
            intended.generated.as_str(),
        ),
        None => format!(
            "no current selection: intended setup {setup} (environment {}, generated {})",
            intended.environment.as_str(),
            intended.generated.as_str(),
        ),
    };
    let hint = apply_hint(invocation.command, scope);
    if json {
        if let Err(exit) = emit_built(
            out,
            notice_event(&NoticeEvent {
                related_command: Some(invocation.command.name().to_owned()),
                ..NoticeEvent::info(
                    "managed_drift",
                    format!("dx {}: {detail}", invocation.command.name()),
                )
            }),
        ) {
            return exit;
        }
        if let Err(exit) = emit_event(out, &command_finished(1, &FinishedCounts::default())) {
            return exit;
        }
        return 1;
    }
    if verbose {
        let written = writeln!(out, "dx {}: {detail}", invocation.command.name())
            .and_then(|()| writeln!(out, "run `{hint}` to select it"));
        if let Err(exit) = check_stdout_write(written) {
            return exit;
        }
    }
    1
}

#[cfg(test)]
mod tests {
    use super::super::managed_codegen::{empty_generated_id, stage_codegen_side};
    use super::super::managed_env::{empty_env_id, stage_env_side};
    use super::super::test_support::*;
    use dx_setup::{read_current_pair, ENVIRONMENTS_DIR_NAME, GENERATED_DIR_NAME};

    #[test]
    fn managed_live_empty_selection_commits_with_empty_counterparts() {
        for command in ["codegen", "env"] {
            let name = format!("managed-commit-{command}");
            let harness = Harness::new(&name);
            let (code, out, err) = harness.run(&[command, "--apply"]);
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
            }
            let (code, out, err) = harness.run(&[command, "--apply"]);
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
            let (code, out, err) = harness.run(&[command, "--apply", "//a:one"]);
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
    fn managed_exact_codegen_expands_bare_schema_to_projections() {
        let harness = Harness::new("managed-expand-check");
        harness.query.script_graph(&prost_fixture_graph());
        let (_, out, err) = harness.run(&["codegen", "//generation:result_proto"]);
        assert!(
            out.contains(
                "Running codegen for //generation:codegen_prost_fixture //generation:result_proto"
            ),
            "{out}{err}"
        );
        assert_eq!(
            harness.query.calls.borrow().len(),
            1,
            "check mode still runs the expansion query"
        );
        assert_eq!(
            harness.query.calls.borrow()[0].last().map(String::as_str),
            Some("kind('rule', //...)"),
            "expansion reads the rule graph"
        );
        for argv in harness.query.calls.borrow().iter() {
            assert!(!argv.iter().any(|arg| arg.contains("rdeps(")), "{argv:?}");
        }

        let harness = Harness::new("managed-expand-live");
        harness.query.script_graph(&prost_fixture_graph());
        let (code, out, err) = harness.run(&["codegen", "--apply", "//generation:result_proto"]);
        assert_eq!(code, 0, "{out}{err}");
        assert!(
            out.contains(
                "Running codegen for //generation:codegen_prost_fixture //generation:result_proto"
            ),
            "{out}"
        );
        assert!(out.contains("selected setup "), "{out}");
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
        let (code, out, err) = harness.run(&["codegen", "--apply", "--quiet"]);
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
        let (code, _, _) = harness.run(&["codegen", "--apply"]);
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
        let (staged, _lease) =
            stage_codegen_side(&workspace, &workspace, &codegen_plan).expect("stage");
        assert_eq!(staged, empty_generated_id());
        let env_plan = dx_env_plan::collect_plan(&[]).expect("empty env plan");
        let (staged, _held) = stage_env_side(&workspace, &env_plan).expect("stage");
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
        let (code, out, err) = harness.run(&["env"]);
        assert_eq!(code, 3, "{out}{err}");
        assert!(out.contains("Running env for //..."), "{out}");
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
        let (code, _, _) = harness.run(&["env", "--", "--aspects=//other.bzl%aspect"]);
        assert_eq!(code, 2);
        assert!(
            harness.seen_env.borrow().is_empty(),
            "policy conflict launches nothing"
        );
    }

    #[test]
    fn managed_live_json_streams_selection() {
        for command in ["codegen", "env"] {
            let name = format!("managed-live-json-{command}");
            let harness = Harness::new(&name);
            let (code, out, err) = harness.run(&[command, "--apply", "--output=json"]);
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
        let (code, out, _) = harness.run(&["env", "--output=json"]);
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
    fn managed_default_reports_missing_selection_without_writing() {
        for command in ["codegen", "env"] {
            let name = format!("managed-check-missing-{command}");
            let harness = Harness::new(&name);
            let (code, out, err) = harness.run(&[command]);
            assert_eq!(code, 1, "{out}{err}");
            assert!(out.contains("Running "), "{out}");
            assert!(out.contains("no current selection"), "{out}");
            assert!(
                out.contains(&format!("dx {command} --apply")),
                "drift names its apply: {out}"
            );
            assert_eq!(err, "", "{err}");
            assert_eq!(
                harness.seen_env.borrow().len(),
                1,
                "{command} check still collects through one Bazel build"
            );
            assert!(
                !harness.workspace.join(".dx").exists(),
                "{command} check stages nothing into the workspace"
            );
            assert_eq!(
                read_current_pair(&harness.workspace).expect("read current"),
                None,
                "{command} check commits no selection"
            );
        }
    }

    #[test]
    fn managed_explicit_check_matches_default() {
        for command in ["codegen", "env"] {
            let default = Harness::new(&format!("managed-check-default-{command}"));
            let (default_code, default_out, _) = default.run(&[command]);
            let explicit = Harness::new(&format!("managed-check-explicit-{command}"));
            let (explicit_code, explicit_out, _) = explicit.run(&[command, "--check"]);
            assert_eq!(default_code, explicit_code, "{command}");
            assert_eq!(default_out, explicit_out, "{command}");
            assert_eq!(default_code, 1, "{command} drift: {default_out}");
        }
    }

    #[test]
    fn managed_check_current_selection_succeeds_without_touching_state() {
        for command in ["codegen", "env"] {
            let name = format!("managed-check-current-{command}");
            let harness = Harness::new(&name);
            let (code, _, err) = harness.run(&[command, "--apply"]);
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
    fn managed_check_stale_selection_reports_drift_without_committing() {
        let harness = Harness::new("managed-check-stale");
        let (code, _, err) = harness.run(&["codegen", "--apply"]);
        assert_eq!(code, 0, "{err}");
        let foreign = dx_setup::SetupPair {
            environment: dx_setup::GenerationId::new(&"3".repeat(64)).expect("fixture id"),
            generated: dx_setup::GenerationId::new(&"4".repeat(64)).expect("fixture id"),
        };
        dx_setup::commit_pair(&harness.workspace, &foreign).expect("foreign commit");
        let (code, out, err) = harness.run(&["codegen"]);
        assert_eq!(code, 1, "{out}{err}");
        assert!(out.contains("stale selection"), "{out}");
        assert!(out.contains("dx codegen --apply"), "{out}");
        assert_eq!(err, "", "{err}");
        assert_eq!(
            read_current_pair(&harness.workspace)
                .expect("reread current")
                .expect("selection kept"),
            foreign,
            "check leaves the stale selection in place"
        );
    }

    #[test]
    fn managed_check_json_emits_drift_notice() {
        let harness = Harness::new("managed-check-json-drift");
        let (code, out, err) = harness.run(&["codegen", "--output=json"]);
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
        assert_eq!(notice["code"], serde_json::json!("managed_drift"));
        assert!(
            notice["message"]
                .as_str()
                .expect("message")
                .contains("no current selection"),
            "{notice}"
        );
        assert!(
            !events
                .iter()
                .any(|event| event["event"] == serde_json::json!("selection")),
            "{out}"
        );
        assert_eq!(
            events.last().expect("finished")["exit_code"],
            serde_json::json!(1)
        );
        assert_eq!(err, "", "{err}");
        assert!(
            !harness.workspace.join(".dx").exists(),
            "json check stages nothing into the workspace"
        );
    }

    #[test]
    fn managed_check_malformed_current_fails_closed() {
        let harness = Harness::new("managed-check-bad-current");
        let generations = harness.workspace.join(".dx").join(GENERATED_DIR_NAME);
        std::fs::create_dir_all(&generations).expect("generations dir");
        let pointer = harness.workspace.join(".dx/setups/current");
        std::fs::create_dir_all(pointer.parent().expect("parent")).expect("setups dir");
        std::fs::write(&pointer, "not a symlink").expect("file pointer");
        let (code, out, err) = harness.run(&["codegen"]);
        assert_eq!(code, 1, "{out}{err}");
        assert!(err.contains("dx: managed_commit_failed:"), "{err}");
        assert!(
            !generations.join(empty_generated_id().as_str()).exists(),
            "malformed current stages nothing into the workspace"
        );
    }

    #[test]
    fn managed_check_exact_scope_reports_drift_without_committing() {
        let harness = Harness::new("managed-check-exact");
        let (code, out, err) = harness.run(&["env", "//a:one"]);
        assert_eq!(code, 1, "{out}{err}");
        assert!(out.contains("Running env for //a:one"), "{out}");
        assert!(out.contains("no current selection"), "{out}");
        assert!(out.contains("dx env --apply //a:one"), "{out}");
        assert_eq!(
            read_current_pair(&harness.workspace).expect("read current"),
            None,
            "exact-scope check commits nothing"
        );
    }
}

#[cfg(test)]
#[path = "managed_delivery_tests.rs"]
mod managed_delivery_tests;
