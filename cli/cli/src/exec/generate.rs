use super::common::*;
use dx_output::{
    change_value, command_finished, command_started, notice_value, FinishedCounts, OutputMode,
};
use dx_plan::args::{GenerateRequest, Invocation};
use dx_plan::finalize::{finalize, FinalizeError, FinalizeInput, GazelleStatus};
use dx_plan::generate::{project, refused_lines, render_diff, text_lines, ProjectedManifest};
use dx_plan::plan::{
    generate_scope_json, intended_path, patch_path, plan_generate, GENERATE_ENV_INTENDED,
    GENERATE_ENV_MODE, GENERATE_ENV_SCOPE,
};
use dx_plan::reports::plan_reports;
use dx_plan::resolve::resolve;
use std::io::Write;

fn bazel_failed_message(code: i32) -> String {
    format!("Bazel generate failed with exit {code}; results are incomplete")
}

fn finish_incomplete_generate(
    output: OutputMode,
    out: &mut dyn Write,
    err: &mut dyn Write,
    code: i32,
) -> i32 {
    if output == OutputMode::Json {
        if code != 0 {
            if let Err(exit) =
                emit_bazel_exit(out, CODE_BAZEL_FAILED, &bazel_failed_message(code), code)
            {
                return kept_child(err, code, exit);
            }
        }
        let finished = command_finished(
            code,
            &FinishedCounts {
                results_complete: Some(false),
                ..FinishedCounts::default()
            },
        );
        if let Err(exit) = emit_event(out, &finished) {
            return kept_child(err, code, exit);
        }
    }
    code
}

pub(crate) fn execute_generate(invocation: &Invocation, env: Env<'_>) -> i32 {
    let request = match invocation.generate_request() {
        Ok(request) => request,
        Err(message) => {
            let Env { out, err, .. } = env;
            return pre_exec(invocation.output, out, err, &message);
        }
    };
    execute_generate_request(&request, env)
}

pub(crate) fn execute_generate_request(request: &GenerateRequest, env: Env<'_>) -> i32 {
    run_generate_request(
        request,
        env,
        &mut PhaseKind::Selection,
        &mut PhaseEffects::default(),
    )
}

pub(crate) fn execute_generate_phase(request: &GenerateRequest, env: Env<'_>) -> PhaseRun {
    let mut kind = PhaseKind::Selection;
    let mut effects = PhaseEffects::default();
    let code = run_generate_request(request, env, &mut kind, &mut effects);
    PhaseRun {
        code,
        kind,
        effects,
    }
}

fn run_generate_request(
    request: &GenerateRequest,
    env: Env<'_>,
    kind: &mut PhaseKind,
    effects: &mut PhaseEffects,
) -> i32 {
    match plan_reports(
        env.workspace,
        dx_plan::args::Command::Generate,
        &request.common.reports,
        &request.common.output,
    ) {
        Ok(_) => {}
        Err(error) => return pre_exec(request.common.output, env.out, env.err, &error.to_string()),
    }
    let resolved = match resolve(
        &request.targets,
        env.workspace,
        env.query_runner,
        &request.bazel_startup_options,
    ) {
        Ok(resolved) => resolved,
        Err(error) => return resolve_failed(request.common.output, env.out, env.err, &error),
    };
    let apply = request.applies();
    let patch = patch_path(env.temp_dir, env.pid, env.nonce);
    let patch_arg = (!apply).then(|| patch.display().to_string());
    let _ = std::fs::remove_file(&patch);
    let plan = match plan_generate(
        &resolved,
        &request.bazel_options,
        !apply,
        &request.bazel_startup_options,
        patch_arg.as_deref(),
    ) {
        Ok(plan) => plan,
        Err(error) => {
            return pre_exec(request.common.output, env.out, env.err, &format!("{error}"))
        }
    };
    *kind = PhaseKind::Tool;
    let mode = if apply { "default" } else { "check" };
    if request.common.output == OutputMode::Json {
        if let Err(exit) = emit_built(env.out, command_started("generate", mode)) {
            return exit;
        }
    } else if request.chatty() {
        if let Err(exit) = check_stdout_write(writeln!(env.out, "{}", plan.summary)) {
            return exit;
        }
    }
    let intended = intended_path(env.temp_dir, env.pid, env.nonce);
    let intended_str = intended.display().to_string();
    let scope_json = generate_scope_json(&resolved);
    let dispatch = [
        (GENERATE_ENV_INTENDED, intended_str.as_str()),
        (GENERATE_ENV_SCOPE, scope_json.as_str()),
        (GENERATE_ENV_MODE, mode),
    ];
    let bazel_code = match run_bazel_output(
        request.common.output,
        env.out,
        env.err,
        env.workspace,
        env.runner,
        &plan.argv,
        &dispatch,
    ) {
        Ok(code) => code,
        Err(exit) => {
            let _ = std::fs::remove_file(&intended);
            let _ = std::fs::remove_file(&patch);
            *kind = PhaseKind::Host;
            return exit;
        }
    };
    let patch_written = std::fs::metadata(&patch).is_ok_and(|meta| meta.len() > 0);
    let _ = std::fs::remove_file(&patch);
    let Some(witness) = std::fs::read(&intended).ok() else {
        if bazel_code != 0 {
            *kind = PhaseKind::from_bazel_exit(bazel_code, false);
            return finish_incomplete_generate(request.common.output, env.out, env.err, bazel_code);
        }
        *kind = PhaseKind::Incomplete;
        return operational_output(
            request.common.output,
            env.out,
            env.err,
            CODE_INVALID_RESULT,
            "generate completed without a result manifest",
        );
    };
    let finalized = finalize(&FinalizeInput {
        intended_json: &witness,
        workspace: env.workspace,
        check: !apply,
        bazel_code,
        patch_written,
    })
    .and_then(|finalized| {
        project(&finalized.manifest)
            .map(|projected| (projected, finalized.gazelle))
            .map_err(FinalizeError::Invalid)
    });
    let (projected, gazelle) = match finalized {
        Ok(result) => result,
        Err(FinalizeError::IncompleteCheck) => {
            *kind = if bazel_code == 0 {
                PhaseKind::Incomplete
            } else {
                PhaseKind::from_bazel_exit(bazel_code, false)
            };
            return finish_incomplete_generate(request.common.output, env.out, env.err, bazel_code);
        }
        Err(error) => {
            *kind = PhaseKind::Incomplete;
            return operational_output(
                request.common.output,
                env.out,
                env.err,
                CODE_INVALID_RESULT,
                &format!("invalid generation manifest: {error}"),
            );
        }
    };
    let code = projected.exit_code(match gazelle {
        GazelleStatus::Failed(failed) => failed,
        GazelleStatus::Completed => 0,
    });
    let next_kind = generate_kind(code, gazelle, projected.results_complete);
    *effects = PhaseEffects {
        changes: projected.files.len() as u64,
        applied: projected.applied(),
        not_applied: projected.not_applied(),
        baseline_refreshed: false,
    };
    if request.common.output == OutputMode::Json {
        for file in projected.sorted_files() {
            let (effect, reason) = ProjectedManifest::effect_of(file);
            let change = change_value(&file.change, effect, reason, request.common.include_edits);
            if let Err(exit) = emit_event(env.out, &change) {
                return kept_child(env.err, bazel_code, exit);
            }
        }
        for notice in &projected.notices {
            if let Err(exit) = emit_event(env.out, &notice_value(notice)) {
                return kept_child(env.err, bazel_code, exit);
            }
        }
        if let GazelleStatus::Failed(failed) = gazelle {
            if let Err(exit) = emit_bazel_exit(
                env.out,
                CODE_BAZEL_FAILED,
                &bazel_failed_message(failed),
                failed,
            ) {
                return kept_child(env.err, bazel_code, exit);
            }
        }
        *kind = next_kind;
        let finished = command_finished(code, &projected.finished_counts());
        if let Err(exit) = emit_event(env.out, &finished) {
            return kept_child(env.err, bazel_code, exit);
        }
        return code;
    }
    if request.common.output == OutputMode::Diff {
        match render_diff(&projected) {
            Ok(patch) => {
                if let Err(exit) = check_stdout_write(env.out.write_all(patch.as_bytes())) {
                    return kept_child(env.err, bazel_code, exit);
                }
            }
            Err(error) => {
                return operational_output(
                    request.common.output,
                    env.out,
                    env.err,
                    CODE_DIFF_FAILED,
                    &format!("failed to render generate patch: {error}"),
                );
            }
        }
    } else {
        for line in text_lines(&projected) {
            if let Err(exit) = check_stdout_write(writeln!(env.out, "{line}")) {
                return kept_child(env.err, bazel_code, exit);
            }
        }
        for line in refused_lines(&projected) {
            let _ = writeln!(env.err, "{line}");
        }
    }
    *kind = next_kind;
    code
}

fn generate_kind(code: i32, gazelle: GazelleStatus, results_complete: bool) -> PhaseKind {
    if let GazelleStatus::Failed(failed) = gazelle {
        PhaseKind::from_bazel_exit(failed, false)
    } else if !results_complete {
        PhaseKind::Incomplete
    } else if code != 0 {
        PhaseKind::Findings
    } else {
        PhaseKind::Clean
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::*;
    use super::*;

    #[test]
    fn generate_runs_repo_wide_and_preserves_bazel_status() {
        let harness = generate_witness("xyz\n", "generate-text");
        let (code, out, err) = harness.run(&["generate", "--apply", "--output=text"]);
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
        let (code, _, _) = failing.run(&["generate", "--apply", "--output=text"]);
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
        let (code, out, err) = harness.run(&["generate", "--apply", "--output=json"]);
        assert_eq!(code, 0, "{out}{err}");
        assert!(out.contains("\"command_started\""), "{out}");
        assert!(out.contains("\"mode\":\"default\""), "{out}");
        assert!(out.contains("\"event\":\"change\""), "{out}");
        assert!(!out.contains("\"event\":\"mutation\""), "{out}");
        assert!(out.contains("\"effect\":\"applied\""), "{out}");
        assert!(!out.contains("\"edits\""), "{out}");
        assert!(out.contains("\"code\":\"ignored_import\""), "{out}");
        assert!(out.contains("\"results_complete\":true"), "{out}");
        assert!(
            out.contains("\"changes\":{\"applied\":1,\"create\":0,\"modify\":1,\"refused\":0}"),
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
        let (code, out, _) = harness.run(&["generate", "--apply", "--output=json"]);
        assert_eq!(code, 2, "{out}");
        assert!(out.contains("\"event\":\"change\""), "{out}");
        assert!(!out.contains("\"effect\":\"proposed\""), "{out}");
        assert!(out.contains("\"results_complete\":false"), "{out}");
        assert!(out.contains("\"exit_code\":2"), "{out}");
        assert!(out.contains("\"code\":\"bazel_failed\""), "{out}");
        assert!(out.contains("\"status\":2"), "{out}");
        let error_at = out.find("\"code\":\"bazel_failed\"").expect("bazel_failed");
        let finished_at = out
            .find("\"event\":\"command_finished\"")
            .expect("command_finished");
        assert!(error_at < finished_at, "{out}");
        let finished: serde_json::Value =
            serde_json::from_str(out.lines().last().expect("last line")).expect("json");
        assert_eq!(finished["event"], "command_finished", "{out}");
        assert_eq!(finished["changes"]["applied"], 1, "{out}");
        assert_eq!(finished["changes"]["refused"], 0, "{out}");
        assert_eq!(finished["changes"]["modify"], 1, "{out}");
    }

    #[test]
    fn generate_partial_default_text_lists_written_files() {
        let mut harness = generate_witness("xyz\n", "generate-partial-text");
        harness.bazel_code = 2;
        let (code, out, err) = harness.run(&["generate", "--apply", "--output=text"]);
        assert_eq!(code, 2, "{out}{err}");
        assert!(
            out.contains("Modified rust/tests/fixtures/hello/BUILD.bazel"),
            "{out}"
        );
    }

    #[test]
    fn generate_not_applied_text_reports_refused_on_stderr() {
        let harness = generate_witness("stale\n", "generate-not-applied-text");
        let (code, out, err) = harness.run(&["generate", "--apply", "--output=text"]);
        assert_eq!(code, 1, "{out}{err}");
        assert!(!out.contains("Modified"), "{out}");
        assert!(
            err.contains("Not applied: rust/tests/fixtures/hello/BUILD.bazel (write_mismatch)"),
            "{err}"
        );
    }

    #[test]
    fn generate_not_applied_mutation_fails() {
        let harness = generate_witness("stale\n", "generate-not-applied");
        let (code, out, err) = harness.run(&["generate", "--apply", "--output=json"]);
        assert_eq!(code, 1, "{out}{err}");
        assert!(out.contains("\"effect\":\"refused\""), "{out}");
        assert!(out.contains("\"reason\":\"write_mismatch\""), "{out}");
        assert!(out.contains("\"exit_code\":1"), "{out}");
    }

    #[test]
    fn generate_kind_follows_the_gazelle_status() {
        use super::GazelleStatus::{Completed, Failed};
        assert_eq!(generate_kind(1, Completed, true), PhaseKind::Findings);
        assert_eq!(generate_kind(1, Completed, false), PhaseKind::Incomplete);
        assert_eq!(generate_kind(1, Failed(1), true), PhaseKind::Incomplete);
        assert_eq!(generate_kind(1, Failed(1), false), PhaseKind::Incomplete);
        assert_eq!(generate_kind(2, Failed(2), true), PhaseKind::Selection);
        assert_eq!(generate_kind(8, Failed(8), true), PhaseKind::Cancelled);
        assert_eq!(generate_kind(36, Failed(36), true), PhaseKind::Tool);
        assert_eq!(generate_kind(0, Completed, true), PhaseKind::Clean);
    }

    fn diff_exit_harness(name: &str, patch: Option<&[u8]>) -> Harness {
        let mut harness = Harness::new(name);
        harness.generate_code = Some(1);
        harness.patch = patch.map(<[u8]>::to_vec);
        harness.intended = Some(intended_witness(
            "check",
            true,
            &intended_modify("rust/tests/fixtures/hello/BUILD.bazel", b"abc\n", b"xyz\n"),
            "",
        ));
        harness
    }

    #[test]
    fn generate_check_exit_one_without_a_patch_is_a_tool_failure() {
        for (name, patch) in [
            ("generate-no-patch", None),
            ("generate-empty-patch", Some(&b""[..])),
        ] {
            let harness = diff_exit_harness(name, patch);
            let (_, out, err) = harness.run(&["generate", "--check", "--output=json"]);
            assert!(out.contains("\"code\":\"bazel_failed\""), "{out}{err}");
        }
    }

    #[test]
    fn generate_check_exit_one_with_a_patch_is_findings() {
        let harness = diff_exit_harness("generate-patch", Some(b"--- a\n+++ b\n"));
        let (code, out, err) = harness.run(&["generate", "--check", "--output=json"]);
        assert_eq!(code, 1, "{out}{err}");
        assert!(!out.contains("bazel_failed"), "{out}");
        assert!(out.contains("\"event\":\"change\""), "{out}");
    }

    #[test]
    fn generate_check_failure_after_a_diff_reports_bazel_failed() {
        let mut harness = diff_exit_harness("generate-diff-exit-two", Some(b"--- a\n+++ b\n"));
        harness.generate_code = Some(2);
        let (code, out, err) = harness.run(&["generate", "--check", "--output=json"]);
        assert_eq!(code, 2, "{out}{err}");
        assert!(out.contains("\"exit_code\":2"), "{out}");
        let error_at = out.find("\"code\":\"bazel_failed\"").expect("bazel_failed");
        let finished_at = out
            .find("\"event\":\"command_finished\"")
            .expect("command_finished");
        assert!(error_at < finished_at, "{out}");
    }

    #[test]
    fn generate_check_exit_one_with_a_clean_witness_is_a_failure() {
        let mut harness = Harness::new("generate-clean-exit-one");
        harness.generate_code = Some(1);
        harness.patch = Some(b"--- a\n+++ b\n".to_vec());
        harness.intended = Some(intended_witness("check", true, "", ""));
        let (code, out, err) = harness.run(&["generate", "--check", "--output=json"]);
        assert_eq!(code, 1, "{out}{err}");
        assert!(out.contains("\"code\":\"bazel_failed\""), "{out}");
        assert!(out.contains("\"exit_code\":1"), "{out}");
    }

    #[test]
    fn generate_witness_result_contradicting_files_fails_closed() {
        let mut harness = Harness::new("generate-contradicting-result");
        harness.intended = Some(intended_witness_with_result(
            "check",
            "clean",
            true,
            &intended_modify("rust/tests/fixtures/hello/BUILD.bazel", b"abc\n", b"xyz\n"),
            "",
        ));
        let (code, out, err) = harness.run(&["generate", "--check", "--output=json"]);
        assert_eq!(code, 1, "{out}{err}");
        assert!(out.contains("\"code\":\"invalid_result\""), "{out}");
        assert!(!out.contains("\"event\":\"change\""), "{out}");
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
        harness.patch = Some(b"--- a\n+++ b\n".to_vec());
        let (code, out, err) = harness.run(&["generate", "--check", "--output=json"]);
        assert_eq!(code, 1, "{out}{err}");
        assert!(!out.contains("bazel_failed"), "{out}");
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
        assert!(!out.contains("\"changes\""), "{out}");
    }

    #[test]
    fn generate_diff_renders_validated_patch_only() {
        let harness = generate_witness("xyz\n", "generate-diff");
        let (code, out, err) = harness.run(&["generate", "--apply", "--output=diff"]);
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
        let (code, out, err) = harness.run(&["generate", "--apply", "--output=diff"]);
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
        let (code, out, err) = harness.run(&["generate", "--apply", "--output=diff"]);
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
        let (code, out, err) = harness.run(&["generate", "--apply", "//a:one", "--output=text"]);
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
    fn generate_quiet_apply_prints_nothing() {
        let mut quiet = Harness::new("generate-quiet");
        quiet.intended = Some(intended_witness("default", true, "", ""));
        let (code, out, _) = quiet.run(&["generate", "--apply", "--quiet"]);
        assert_eq!(code, 0, "{out}");
        assert_eq!(out, "", "{out:?}");
    }

    #[test]
    fn generate_scoped_run_resolves_labels_without_queries() {
        let harness = generate_witness("xyz\n", "generate-scoped");
        let (code, out, err) = harness.run(&["generate", "--apply", "//a:one", "--output=text"]);
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

    #[test]
    fn generate_default_checks_without_writing() {
        let mut harness = Harness::new("generate-default-check");
        harness.write_source("rust/tests/fixtures/hello/BUILD.bazel", "xyz\n");
        harness.intended = Some(intended_witness(
            "check",
            true,
            &intended_modify("rust/tests/fixtures/hello/BUILD.bazel", b"abc\n", b"xyz\n"),
            "",
        ));
        let (code, out, err) = harness.run(&["generate", "--output=text"]);
        assert_eq!(code, 1, "{out}{err}");
        assert!(out.contains("Running generate for //..."), "{out}");
        assert!(
            out.contains("Modified rust/tests/fixtures/hello/BUILD.bazel"),
            "{out}"
        );
        assert_eq!(
            std::fs::read(
                harness
                    .workspace
                    .join("rust/tests/fixtures/hello/BUILD.bazel")
            )
            .expect("source"),
            b"xyz\n",
            "default check leaves sources untouched"
        );
        assert_eq!(err, "", "{err}");
    }

    #[test]
    fn generate_default_check_json_reports_changes_without_mutations() {
        let mut harness = Harness::new("generate-default-check-json");
        harness.intended = Some(intended_witness(
            "check",
            true,
            &intended_modify("rust/tests/fixtures/hello/BUILD.bazel", b"abc\n", b"xyz\n"),
            "",
        ));
        let (code, out, err) = harness.run(&["generate", "--output=json"]);
        assert_eq!(code, 1, "{out}{err}");
        assert!(out.contains("\"mode\":\"check\""), "{out}");
        assert!(out.contains("\"event\":\"change\""), "{out}");
        assert!(!out.contains("\"event\":\"mutation\""), "{out}");
        assert!(out.contains("\"exit_code\":1"), "{out}");
    }

    fn generate_phase(harness: &Harness, words: &[&str]) -> PhaseRun {
        let parsed = dx_plan::args::parse(&dx_testing::cli::strings(words)).expect("parse");
        let inv =
            dx_plan::args::apply_here(&parsed, &harness.workspace, &harness.cwd).expect("here");
        let request = inv.generate_request().expect("generate request");
        let runner = harness.runner();
        let mut out = Vec::new();
        let mut err = Vec::new();
        execute_generate_phase(&request, harness.env(&runner, &mut out, &mut err))
    }

    #[test]
    fn generate_apply_phase_reports_its_modify_as_touched() {
        let harness = generate_witness("xyz\n", "generate-phase-effects");
        let run = generate_phase(&harness, &["generate", "--apply", "--output=text"]);
        assert_eq!(run.code, 0);
        assert_eq!(run.effects.changes, 1);
        assert_eq!(run.effects.applied, 1);
        assert_eq!(run.effects.not_applied, 0);
        assert!(run.effects.touched(), "{:?}", run.effects);
    }

    #[test]
    fn generate_launch_failure_and_signal_end_the_phase_as_host() {
        let mut launch_failed = Harness::new("generate-phase-launchfail");
        launch_failed.io_error = true;
        let mut signalled = Harness::new("generate-phase-signalled");
        signalled.signalled = true;
        for harness in [launch_failed, signalled] {
            let run = generate_phase(&harness, &["generate", "--output=text"]);
            assert_eq!(run.code, 1);
            assert_eq!(run.kind, PhaseKind::Host);
        }
    }

    #[test]
    fn generate_interrupted_without_manifest_ends_the_phase_as_cancelled() {
        let mut harness = Harness::new("generate-phase-interrupted");
        harness.generate_code = Some(8);
        let run = generate_phase(&harness, &["generate", "--output=text"]);
        assert_eq!(run.kind, PhaseKind::Cancelled);
    }

    #[test]
    fn generate_apply_phase_with_no_changes_is_untouched() {
        let mut harness = generate_witness("xyz\n", "generate-phase-untouched");
        harness.intended = Some(intended_witness("default", true, "", ""));
        let run = generate_phase(&harness, &["generate", "--apply", "--output=text"]);
        assert_eq!(run.code, 0);
        assert_eq!(run.kind, PhaseKind::Clean);
        assert_eq!(run.effects, PhaseEffects::default());
        assert!(!run.effects.touched());
    }

    #[test]
    fn generate_explicit_check_matches_default() {
        let witness = || {
            intended_witness(
                "check",
                true,
                &intended_modify("rust/tests/fixtures/hello/BUILD.bazel", b"abc\n", b"xyz\n"),
                "",
            )
        };
        let mut default = Harness::new("generate-default-matches");
        default.intended = Some(witness());
        let (default_code, default_out, _) = default.run(&["generate", "--output=text"]);
        let mut explicit = Harness::new("generate-explicit-matches");
        explicit.intended = Some(witness());
        let (explicit_code, explicit_out, _) =
            explicit.run(&["generate", "--check", "--output=text"]);
        assert_eq!(default_code, explicit_code);
        assert_eq!(default_out, explicit_out);
        assert_eq!(default_code, 1, "{default_out}");
    }

    #[test]
    fn generate_apply_writes_after_default_check_reports_drift() {
        let witness = || {
            intended_witness(
                "check",
                true,
                &intended_modify("rust/tests/fixtures/hello/BUILD.bazel", b"abc\n", b"xyz\n"),
                "",
            )
        };
        let mut harness = Harness::new("generate-apply-after-check");
        harness.write_source("rust/tests/fixtures/hello/BUILD.bazel", "xyz\n");
        harness.intended = Some(witness());
        let (code, _, _) = harness.run(&["generate", "--output=text"]);
        assert_eq!(code, 1);
        harness.intended = Some(intended_witness(
            "default",
            true,
            &intended_modify("rust/tests/fixtures/hello/BUILD.bazel", b"abc\n", b"xyz\n"),
            "",
        ));
        let (code, out, err) = harness.run(&["generate", "--apply", "--output=text"]);
        assert_eq!(code, 0, "{out}{err}");
        assert!(
            out.contains("Modified rust/tests/fixtures/hello/BUILD.bazel"),
            "{out}"
        );
    }
}

#[cfg(test)]
#[path = "generate_delivery_tests.rs"]
mod generate_delivery_tests;
