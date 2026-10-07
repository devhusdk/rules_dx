use super::common::*;
use crate::args::Invocation;
use crate::finalize::{finalize, FinalizeError, FinalizeInput};
use crate::generate::{project, render_diff, text_lines};
use crate::plan::{
    generate_scope_json, intended_path, plan_generate, GENERATE_ENV_INTENDED, GENERATE_ENV_MODE,
    GENERATE_ENV_SCOPE,
};
use crate::reports::plan_reports;
use crate::resolve::resolve;
use dx_output::{
    change_value, command_finished, command_started, mutation_value, notice_value, write_event,
    FinishedCounts, OutputMode,
};
use std::io::Write;

fn finish_incomplete_generate(invocation: &Invocation, out: &mut dyn Write, code: i32) -> i32 {
    if invocation.output == OutputMode::Json {
        let _ = write_event(
            out,
            &command_finished(
                code,
                &FinishedCounts {
                    results_complete: Some(false),
                    ..FinishedCounts::default()
                },
            ),
        );
    }
    code
}

pub(crate) fn execute_generate(invocation: &Invocation, env: Env<'_>) -> i32 {
    match plan_reports(
        invocation.command,
        &invocation.reports,
        &invocation.output,
        invocation.dry_run,
    ) {
        Ok(_) => {}
        Err(error) => return pre_exec(env.err, &error.to_string()),
    }
    let resolved = match resolve(&invocation.targets, env.workspace, env.query_runner) {
        Ok(resolved) => resolved,
        Err(error) => return pre_exec(env.err, &error.to_string()),
    };
    let plan = match plan_generate(&resolved, &invocation.bazel_options, invocation.check) {
        Ok(plan) => plan,
        Err(error) => return pre_exec(env.err, &format!("{error}")),
    };
    let mode = if invocation.check { "check" } else { "default" };
    if invocation.dry_run {
        if invocation.output == OutputMode::Json {
            if let Ok(event) = command_started(invocation.command.name(), true, mode) {
                let _ = write_event(env.out, &event);
            }
            let _ = write_event(env.out, &command_finished(0, &FinishedCounts::default()));
        } else if invocation.chatty() {
            let _ = writeln!(env.out, "{}", plan.summary);
        }
        return 0;
    }
    if invocation.output == OutputMode::Json {
        if let Ok(event) = command_started(invocation.command.name(), false, mode) {
            let _ = write_event(env.out, &event);
        }
    } else if invocation.chatty() {
        let _ = writeln!(env.out, "{}", plan.summary);
    }
    let intended = intended_path(env.temp_dir, env.pid, env.nonce);
    let intended_str = intended.display().to_string();
    let scope_json = generate_scope_json(&resolved);
    let dispatch = [
        (GENERATE_ENV_INTENDED, intended_str.as_str()),
        (GENERATE_ENV_SCOPE, scope_json.as_str()),
        (GENERATE_ENV_MODE, mode),
    ];
    let bazel_code = match run_bazel(
        invocation,
        env.out,
        env.err,
        env.workspace,
        env.runner,
        &plan.argv,
        &dispatch,
    ) {
        Ok(code) => code,
        Err(exit) => return exit,
    };
    let Some(witness) = std::fs::read(&intended).ok() else {
        if bazel_code != 0 {
            return finish_incomplete_generate(invocation, env.out, bazel_code);
        }
        return operational(
            invocation,
            env.out,
            env.err,
            CODE_INVALID_RESULT,
            "generate completed without a result manifest",
        );
    };
    let finalized = finalize(&FinalizeInput {
        intended_json: &witness,
        workspace: env.workspace,
        check: invocation.check,
        gazelle_ok: bazel_code == 0,
    })
    .and_then(|manifest| project(&manifest).map_err(FinalizeError::Invalid));
    let projected = match finalized {
        Ok(projected) => projected,
        Err(FinalizeError::IncompleteCheck) => {
            return finish_incomplete_generate(invocation, env.out, bazel_code);
        }
        Err(error) => {
            return operational(
                invocation,
                env.out,
                env.err,
                CODE_INVALID_RESULT,
                &format!("invalid generation manifest: {error}"),
            );
        }
    };
    if invocation.output == OutputMode::Json {
        for file in projected.sorted_files() {
            let _ = write_event(env.out, &change_value(&file.change));
        }
        if !invocation.check {
            for mutation in projected.mutations() {
                let _ = write_event(
                    env.out,
                    &mutation_value(
                        mutation.path,
                        mutation.kind,
                        mutation.outcome,
                        mutation.reason,
                    ),
                );
            }
        }
        for notice in &projected.notices {
            let _ = write_event(env.out, &notice_value(notice));
        }
        let code = projected.exit_code(bazel_code);
        let _ = write_event(
            env.out,
            &command_finished(code, &projected.finished_counts()),
        );
        return code;
    }
    if invocation.output == OutputMode::Diff {
        match render_diff(&projected) {
            Ok(patch) => {
                env.out.write_all(patch.as_bytes()).ok();
            }
            Err(error) => {
                return operational(
                    invocation,
                    env.out,
                    env.err,
                    CODE_DIFF_FAILED,
                    &format!("failed to render generate patch: {error}"),
                );
            }
        }
    } else {
        for line in text_lines(&projected) {
            let _ = writeln!(env.out, "{line}");
        }
    }
    projected.exit_code(bazel_code)
}

#[cfg(test)]
mod tests {
    use super::super::test_support::*;
    use super::*;

    #[test]
    fn generate_runs_repo_wide_and_preserves_bazel_status() {
        let harness = generate_witness("xyz\n", "generate-text");
        let (code, out, err) = harness.run(&["generate", "--output=text"]);
        assert_eq!(code, 0, "{err}");
        assert!(out.contains("Running generate for //..."), "{out}");
        assert!(
            out.contains("Modified rust/tests/fixtures/hello/BUILD.bazel"),
            "{out}"
        );
        assert_eq!(err, "", "{err}");
        assert!(
            harness.query.calls.borrow().is_empty(),
            "repo-wide generate issues no ownership queries"
        );

        let mut failing = generate_witness("xyz\n", "generate-fails");
        failing.bazel_code = 2;
        let (code, _, _) = failing.run(&["generate", "--output=text"]);
        assert_eq!(code, 2);
    }

    #[test]
    fn generate_json_reports_changes_mutations_and_notices() {
        let mut harness = Harness::new("generate-json");
        harness.write_source("rust/tests/fixtures/hello/BUILD.bazel", "xyz\n");
        harness.intended = Some(intended_witness(
            "default",
            true,
            &intended_modify("rust/tests/fixtures/hello/BUILD.bazel", b"abc\n", b"xyz\n"),
            &intended_ignored("rust/tests/fixtures/hello/BUILD.bazel", "rust", "serde"),
        ));
        let (code, out, err) = harness.run(&["generate", "--output=json"]);
        assert_eq!(code, 0, "{out}{err}");
        assert!(out.contains("\"command_started\""), "{out}");
        assert!(out.contains("\"mode\":\"default\""), "{out}");
        assert!(out.contains("\"event\":\"change\""), "{out}");
        assert!(out.contains("\"event\":\"mutation\""), "{out}");
        assert!(out.contains("\"outcome\":\"applied\""), "{out}");
        assert!(out.contains("\"code\":\"ignored_import\""), "{out}");
        assert!(out.contains("\"results_complete\":true"), "{out}");
        assert!(
            out.contains("\"changes\":{\"create\":0,\"modify\":1}"),
            "{out}"
        );
        assert!(
            out.contains("\"mutations\":{\"applied\":1,\"not_applied\":0}"),
            "{out}"
        );
        assert!(out.contains("\"exit_code\":0"), "{out}");
    }

    #[test]
    fn generate_failure_without_witness_stays_incomplete() {
        for (name, bazel_code, exit_code) in
            [("generate-json-ok", 0, 1), ("generate-json-fail", 2, 2)]
        {
            let mut harness = Harness::new(name);
            harness.bazel_code = bazel_code;
            let (code, out, _) = harness.run(&["generate", "--output=json"]);
            assert_eq!(code, exit_code, "{out}");
            assert!(out.contains("\"command_started\""), "{out}");
            assert!(out.contains("\"command_finished\""), "{out}");
            assert!(out.contains("\"results_complete\":false"), "{out}");
            assert!(!out.contains("\"changes\""), "{out}");
            assert!(!out.contains("\"mutations\""), "{out}");
            assert!(!out.contains("\"event\":\"change\""), "{out}");
            assert!(!out.contains("\"event\":\"mutation\""), "{out}");
            if bazel_code == 0 {
                assert!(out.contains("\"code\":\"invalid_result\""), "{out}");
            }
        }
    }

    #[test]
    fn generate_success_without_witness_fails_closed_in_text() {
        let harness = Harness::new("generate-no-witness");
        let (code, out, err) = harness.run(&["generate", "--output=text"]);
        assert_eq!(code, 1, "{out}");
        assert!(err.contains("invalid_result"), "{err}");
        assert!(err.contains("without a result manifest"), "{err}");
        assert!(!out.contains("Modified"), "{out}");
    }

    #[test]
    fn generate_malformed_witness_fails_closed() {
        let mut harness = Harness::new("generate-malformed");
        harness.intended = Some(b"not json".to_vec());
        let (code, out, err) = harness.run(&["generate", "--output=json"]);
        assert_eq!(code, 1, "{out}");
        assert!(err.contains("invalid_result"), "{err}");
        assert!(out.contains("\"code\":\"invalid_result\""), "{out}");
        assert!(!out.contains("\"event\":\"change\""), "{out}");
        assert!(!out.contains("\"event\":\"mutation\""), "{out}");
        assert!(out.contains("\"results_complete\":false"), "{out}");
    }

    #[test]
    fn generate_invalid_witness_fails_closed() {
        let mut harness = Harness::new("generate-invalid");
        harness.intended = Some(intended_witness("check", false, "", ""));
        let (code, _, err) = harness.run(&["generate", "--check", "--output=text"]);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("invalid_result"), "{err}");
    }

    #[test]
    fn generate_partial_default_reports_attempted_prefix() {
        let mut harness = generate_witness("xyz\n", "generate-partial");
        harness.bazel_code = 2;
        let (code, out, _) = harness.run(&["generate", "--output=json"]);
        assert_eq!(code, 2, "{out}");
        assert!(out.contains("\"event\":\"change\""), "{out}");
        assert!(out.contains("\"event\":\"mutation\""), "{out}");
        assert!(out.contains("\"results_complete\":false"), "{out}");
        assert!(out.contains("\"exit_code\":2"), "{out}");
    }

    #[test]
    fn generate_not_applied_mutation_fails() {
        let harness = generate_witness("stale\n", "generate-not-applied");
        let (code, out, err) = harness.run(&["generate", "--output=json"]);
        assert_eq!(code, 1, "{out}{err}");
        assert!(out.contains("\"outcome\":\"not_applied\""), "{out}");
        assert!(out.contains("write_mismatch"), "{out}");
        assert!(out.contains("\"exit_code\":1"), "{out}");
    }

    #[test]
    fn generate_check_reports_changes_without_mutations() {
        let mut harness = Harness::new("generate-check-json");
        harness.intended = Some(intended_witness(
            "check",
            true,
            &intended_modify("rust/tests/fixtures/hello/BUILD.bazel", b"abc\n", b"xyz\n"),
            &intended_ignored("rust/tests/fixtures/hello/BUILD.bazel", "rust", "serde"),
        ));
        let (code, out, err) = harness.run(&["generate", "--check", "--output=json"]);
        assert_eq!(code, 1, "{out}{err}");
        assert!(out.contains("\"mode\":\"check\""), "{out}");
        assert!(out.contains("\"event\":\"change\""), "{out}");
        assert!(!out.contains("\"event\":\"mutation\""), "{out}");
        assert!(out.contains("\"code\":\"ignored_import\""), "{out}");
        assert!(!out.contains("\"mutations\""), "{out}");
        assert!(out.contains("\"exit_code\":1"), "{out}");
    }

    #[test]
    fn generate_check_reports_changes_despite_diff_exit() {
        let mut harness = Harness::new("generate-check-diff-exit");
        harness.intended = Some(intended_witness(
            "check",
            true,
            &intended_modify("rust/tests/fixtures/hello/BUILD.bazel", b"abc\n", b"xyz\n"),
            &intended_ignored("rust/tests/fixtures/hello/BUILD.bazel", "rust", "serde"),
        ));
        harness.bazel_code = 1;
        let (code, out, err) = harness.run(&["generate", "--check", "--output=json"]);
        assert_eq!(code, 1, "{out}{err}");
        assert!(out.contains("\"mode\":\"check\""), "{out}");
        assert!(out.contains("\"event\":\"change\""), "{out}");
        assert!(!out.contains("\"event\":\"mutation\""), "{out}");
        assert!(out.contains("\"code\":\"ignored_import\""), "{out}");
        assert!(!out.contains("\"mutations\""), "{out}");
        assert!(out.contains("\"exit_code\":1"), "{out}");
    }

    #[test]
    fn generate_check_text_lists_changes_and_clean_succeeds() {
        let mut harness = Harness::new("generate-check-text");
        harness.intended = Some(intended_witness(
            "check",
            true,
            &intended_modify("rust/tests/fixtures/hello/BUILD.bazel", b"abc\n", b"xyz\n"),
            "",
        ));
        let (code, out, err) = harness.run(&["generate", "--check", "--output=text"]);
        assert_eq!(code, 1, "{err}");
        assert!(out.contains("Running generate for //..."), "{out}");
        assert!(
            out.contains("Modified rust/tests/fixtures/hello/BUILD.bazel"),
            "{out}"
        );

        let mut clean = Harness::new("generate-check-clean");
        clean.intended = Some(intended_witness("check", true, "", ""));
        let (code, out, err) = clean.run(&["generate", "--check", "--output=text"]);
        assert_eq!(code, 0, "{out}{err}");
        assert!(out.contains("Running generate for //..."), "{out}");
        assert!(!out.contains("Modified"), "{out}");
    }

    #[test]
    fn generate_incomplete_check_reports_nothing() {
        let mut harness = Harness::new("generate-incomplete-check");
        harness.write_source("rust/tests/fixtures/hello/BUILD.bazel", "xyz\n");
        harness.intended = Some(intended_witness(
            "check",
            false,
            &intended_modify("rust/tests/fixtures/hello/BUILD.bazel", b"abc\n", b"xyz\n"),
            "",
        ));
        harness.bazel_code = 2;
        let (code, out, _) = harness.run(&["generate", "--check", "--output=json"]);
        assert_eq!(code, 2, "{out}");
        assert!(!out.contains("\"event\":\"change\""), "{out}");
        assert!(!out.contains("\"event\":\"mutation\""), "{out}");
        assert!(out.contains("\"results_complete\":false"), "{out}");
    }

    #[test]
    fn generate_diff_renders_validated_patch_only() {
        let harness = generate_witness("xyz\n", "generate-diff");
        let (code, out, err) = harness.run(&["generate", "--output=diff"]);
        assert_eq!(code, 0, "{err}");
        assert!(
            out.contains("rust/tests/fixtures/hello/BUILD.bazel"),
            "{out}"
        );
        assert!(out.contains("-abc"), "{out}");
        assert!(out.contains("+xyz"), "{out}");
        assert!(!out.contains("Running generate"), "{out}");
        assert!(!out.contains("Ignored import"), "{out}");
        assert_eq!(err, "", "{err}");
    }

    #[test]
    fn generate_diff_reports_an_empty_create_instead_of_an_unappliable_patch() {
        let mut harness = Harness::new("generate-diff-empty-create");
        harness.intended = Some(intended_witness(
            "default",
            true,
            &intended_create("new/BUILD.bazel", b""),
            "",
        ));
        let (code, out, err) = harness.run(&["generate", "--output=diff"]);
        assert_eq!(code, 1, "{out}{err}");
        assert_eq!(out, "", "{out}");
        assert!(
            err.contains("cannot create empty \"new/BUILD.bazel\""),
            "{err}"
        );
        assert!(err.contains(CODE_DIFF_FAILED), "{err}");
    }

    #[test]
    fn generate_diff_renders_a_non_empty_create() {
        let mut harness = Harness::new("generate-diff-create");
        harness.write_source("new/BUILD.bazel", "# created\n");
        harness.intended = Some(intended_witness(
            "default",
            true,
            &intended_create("new/BUILD.bazel", b"# created\n"),
            "",
        ));
        let (code, out, err) = harness.run(&["generate", "--output=diff"]);
        assert_eq!(code, 0, "{out}{err}");
        assert_eq!(
            out,
            "--- /dev/null\n+++ b/new/BUILD.bazel\n@@ -0,0 +1,1 @@\n+# created\n"
        );
        assert_eq!(err, "", "{err}");
    }

    #[test]
    fn generate_dispatch_env_carries_scope_mode_and_witness() {
        let harness = generate_witness("xyz\n", "generate-dispatch");
        let (code, out, err) = harness.run(&["generate", "//a:one", "--output=text"]);
        assert_eq!(code, 0, "{out}{err}");
        let seen = harness.seen_env.borrow();
        assert_eq!(seen.len(), 1, "{seen:?}");
        let env: std::collections::HashMap<&str, &str> = seen[0]
            .iter()
            .map(|(key, value)| (key.as_str(), value.as_str()))
            .collect();
        assert_eq!(
            env.get(GENERATE_ENV_SCOPE).copied(),
            Some(r#"[{"dirs":["a"],"element":"//a:one"}]"#),
            "{seen:?}"
        );
        assert_eq!(env.get(GENERATE_ENV_MODE).copied(), Some("default"));
        let intended = env
            .get(GENERATE_ENV_INTENDED)
            .copied()
            .expect("witness path");
        assert!(intended.ends_with(".json"), "{intended}");
        assert_eq!(
            std::fs::read(intended).expect("witness bytes"),
            harness.intended.expect("canned witness"),
            "runner observes the dispatched witness path"
        );

        let mut check = Harness::new("generate-dispatch-check");
        check.intended = Some(intended_witness("check", true, "", ""));
        let (code, _, err) = check.run(&["generate", "--check", "--output=text"]);
        assert_eq!(code, 0, "{err}");
        let seen = check.seen_env.borrow();
        assert_eq!(
            seen[0]
                .iter()
                .find_map(|(key, value)| (*key == GENERATE_ENV_MODE).then_some(value.as_str())),
            Some("check"),
            "{seen:?}"
        );
    }

    #[test]
    fn generate_dry_run_plans_without_launching() {
        let mut harness = Harness::new("generate-dryrun");
        harness.bazel_code = 3;
        let (code, out, _) = harness.run(&["generate", "--dry-run", "--output=text"]);
        assert_eq!(code, 0, "{out}");
        assert!(out.contains("Running generate for //..."), "{out}");

        let harness = Harness::new("generate-dryrun-json");
        let (code, out, _) = harness.run(&["generate", "--dry-run", "--output=json"]);
        assert_eq!(code, 0, "{out}");
        assert!(out.contains("\"command_started\""), "{out}");
        assert!(out.contains("\"command_finished\""), "{out}");

        let mut quiet = Harness::new("generate-quiet");
        quiet.intended = Some(intended_witness("default", true, "", ""));
        let (code, out, _) = quiet.run(&["generate", "--quiet"]);
        assert_eq!(code, 0, "{out}");
        assert_eq!(out, "", "{out:?}");
    }

    #[test]
    fn generate_scoped_run_resolves_labels_without_queries() {
        let harness = generate_witness("xyz\n", "generate-scoped");
        let (code, out, err) = harness.run(&["generate", "//a:one", "--output=text"]);
        assert_eq!(code, 0, "{err}");
        assert!(out.contains("Running generate for //a:one"), "{out}");
        assert!(
            harness.query.calls.borrow().is_empty(),
            "label scope issues no ownership queries"
        );
    }

    #[test]
    fn generate_unresolvable_scope_fails_pre_exec() {
        let harness = Harness::new("generate-bad-scope");
        let (code, _, err) = harness.run(&["generate", "no/such/dir"]);
        assert_eq!(code, 2, "{err}");
    }

    #[test]
    fn generate_reports_and_conflicts_fail_pre_exec() {
        let harness = Harness::new("generate-report");
        let (code, _, err) = harness.run(&["generate", "--report=sarif=out.sarif"]);
        assert_eq!(code, 2, "{err}");
        assert!(err.contains("is not supported by dx generate"), "{err}");

        let harness = Harness::new("generate-conflict");
        let (code, _, err) = harness.run(&["generate", "--", "--home_rc"]);
        assert_eq!(code, 2, "{err}");
    }

    #[test]
    fn generate_launch_failure_and_signal_are_operational() {
        let mut harness = Harness::new("generate-launchfail");
        harness.io_error = true;
        let (code, out, err) = harness.run(&["generate", "--output=json"]);
        assert_eq!(code, 1, "{err}");
        assert!(out.contains("launch_failed"), "{out}");

        let mut harness = Harness::new("generate-signalled");
        harness.signalled = true;
        let (code, _, err) = harness.run(&["generate", "--output=text"]);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("bazel_signalled"), "{err}");
    }
}
