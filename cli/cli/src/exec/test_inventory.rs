use std::io::Write;

use crate::args::{Command, Invocation};
use crate::exec::common::{check_stdout_write, emit_event, operational, pre_exec};
use crate::resolve::{first_line, resolve_for_test_with_selection, QueryRunner, SelectionContext};
use dx_output::{
    command_finished, command_started, error_event, status_event, FinishedCounts, OutputMode,
    StatusEvent,
};
use dx_process::operational_code;

pub(crate) const CODE_NO_TESTS: &str = "no_tests";
pub(crate) const CODE_QUERY_FAILED: &str = "bazel_failed";
pub(crate) const CODE_UNSUPPORTED: &str = "unsupported_framework";
pub(crate) const CODE_NO_CASES: &str = "no_cases";
pub(crate) const CODE_CASE_FAILED: &str = "case_failed";
pub(crate) const CODE_TRUNCATED: &str = "cases_truncated";

const SUPPORTED_CASE_KINDS: [&str; 2] = ["rust_test", "_rust_forward_test"];
const MAX_CASES_PER_TARGET: usize = 2000;
const MAX_CASES_TOTAL: usize = 5000;

struct Ctx<'a> {
    invocation: &'a Invocation,
    verb: &'a str,
    selection: SelectionContext,
    workspace: &'a std::path::Path,
    query_runner: &'a dyn QueryRunner,
    out: &'a mut dyn Write,
    err: &'a mut dyn Write,
    is_json: bool,
}

fn selection_for(invocation: &Invocation) -> SelectionContext {
    if invocation.configured {
        SelectionContext::for_tests_configured(
            &invocation.bazel_options,
            &invocation.bazel_startup_options,
        )
    } else {
        SelectionContext::unconfigured_with_options(
            &invocation.bazel_options,
            &invocation.bazel_startup_options,
        )
    }
}

fn inventory_expression(scope: &str) -> String {
    format!("tests({scope})")
}

fn query_argv(selection: &SelectionContext, verb: &str, expr: &str) -> Result<Vec<String>, String> {
    selection
        .selection_argv(verb, &["--output=label_kind".to_owned()], expr)
        .map_err(|error| error.to_string())
}

fn list_argv(ctx: &Ctx, label: &str) -> Result<Vec<String>, String> {
    let protected = crate::plan::workflow_protected(crate::plan::WorkflowVerb::Run);
    let filtered = ctx
        .selection
        .filtered_options()
        .map_err(|error| error.to_string())?;
    let mut argv = dx_process::build_workflow_argv(
        "run",
        &filtered,
        &[],
        &protected,
        &[label.to_owned()],
        &ctx.invocation.bazel_startup_options,
    )
    .map_err(|error| error.to_string())?;
    argv.push("--".to_owned());
    argv.push("--list".to_owned());
    Ok(argv)
}

fn parse_label_kind(stdout: &[u8]) -> Result<Vec<(String, String)>, String> {
    let text = std::str::from_utf8(stdout).map_err(|_| "query output is not UTF-8".to_owned())?;
    let mut pairs: Vec<(String, String)> = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let mut parts = line.split_whitespace();
        match (parts.next(), parts.next(), parts.next()) {
            (Some(kind), Some(rule), Some(label)) if rule == "rule" && label.starts_with("//") => {
                pairs.push((kind.to_owned(), label.to_owned()));
            }
            _ => {
                return Err(format!(
                    "query returned a malformed test observation: {line}"
                ))
            }
        }
    }
    pairs.sort_by(|left, right| left.1.cmp(&right.1).then(left.0.cmp(&right.0)));
    pairs.dedup_by(|left, right| left.1 == right.1);
    Ok(pairs)
}

fn parse_cases(stdout: &[u8]) -> Vec<String> {
    let text = String::from_utf8_lossy(stdout);
    let mut cases: Vec<String> = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if let Some(name) = line.strip_suffix(": test") {
            let name = name.trim();
            if !name.is_empty() {
                cases.push(name.to_owned());
            }
        }
    }
    cases.sort();
    cases.dedup();
    cases
}

fn run_inventory_query(ctx: &Ctx, expr: &str) -> Result<Vec<(String, String)>, (String, String)> {
    let argv = query_argv(&ctx.selection, ctx.verb, expr)
        .map_err(|detail| (CODE_QUERY_FAILED.to_owned(), detail))?;
    let result = ctx
        .query_runner
        .run_query(&argv, ctx.workspace)
        .map_err(|error| (CODE_QUERY_FAILED.to_owned(), error.to_string()))?;
    if result.code != Some(0) {
        return Err((
            CODE_QUERY_FAILED.to_owned(),
            format!(
                "query failed: bazel {} {expr} exited with code {} ({})",
                ctx.verb,
                result.code.unwrap_or(-1),
                first_line(&result.stderr)
            ),
        ));
    }
    parse_label_kind(&result.stdout).map_err(|detail| (CODE_QUERY_FAILED.to_owned(), detail))
}

pub(crate) fn execute_tests(invocation: &Invocation, env: crate::exec::Env<'_>) -> i32 {
    let crate::exec::Env {
        workspace,
        query_runner,
        out,
        err,
        ..
    } = env;
    if invocation.command != Command::Tests {
        return pre_exec(err, "not a tests command");
    }
    let verb = if invocation.configured {
        "cquery"
    } else {
        "query"
    };
    let selection = selection_for(invocation);
    let mut ctx = Ctx {
        invocation,
        verb,
        selection,
        workspace,
        query_runner,
        out,
        err,
        is_json: invocation.output == OutputMode::Json,
    };
    if invocation.dry_run {
        return dry_run_tests(&mut ctx);
    }
    let resolved = match resolve_for_test_with_selection(
        &invocation.targets,
        ctx.workspace,
        ctx.query_runner,
        &invocation.bazel_startup_options,
        &ctx.selection,
    ) {
        Ok(resolved) => resolved,
        Err(error) => return pre_exec(ctx.err, &error.to_string()),
    };
    let mut pairs: Vec<(String, String, String)> = Vec::new();
    let mut empty_scopes: Vec<String> = Vec::new();
    for scope in &resolved.targets {
        let expr = inventory_expression(scope);
        match run_inventory_query(&ctx, &expr) {
            Ok(found) => {
                if found.is_empty() {
                    empty_scopes.push(scope.clone());
                }
                for (kind, label) in found {
                    pairs.push((kind, label, scope.clone()));
                }
            }
            Err((code, detail)) => {
                return operational(ctx.invocation, ctx.out, ctx.err, &code, &detail);
            }
        }
    }
    pairs.sort_by(|left, right| {
        left.1
            .cmp(&right.1)
            .then(left.0.cmp(&right.0))
            .then(left.2.cmp(&right.2))
    });
    pairs.dedup_by(|left, right| left.1 == right.1);
    if pairs.is_empty() {
        let mut scopes: Vec<String> = resolved.targets.clone();
        scopes.sort();
        scopes.dedup();
        let verb = ctx.verb;
        return operational(
            ctx.invocation,
            ctx.out,
            ctx.err,
            CODE_NO_TESTS,
            &format!(
                "no tests for {} via bazel {verb}: pass an explicit test label or pattern such as //pkg/...",
                scopes.join(" ")
            ),
        );
    }
    if !ctx.invocation.cases {
        let code = if empty_scopes.is_empty() { 0 } else { 1 };
        if ctx.is_json {
            return emit_inventory_json(&mut ctx, &pairs, &empty_scopes, code);
        }
        return emit_inventory_text(&mut ctx, &pairs, &empty_scopes);
    }
    emit_cases(&mut ctx, &pairs, &empty_scopes)
}

fn dry_run_tests(ctx: &mut Ctx) -> i32 {
    let mut cache = crate::resolve::PackageCache::default();
    let classified =
        match crate::resolve::classify_scopes(&ctx.invocation.targets, ctx.workspace, &mut cache) {
            Ok(classified) => classified,
            Err(error) => return pre_exec(ctx.err, &error.to_string()),
        };
    let mut exprs: Vec<String> = Vec::new();
    if ctx.invocation.targets.is_empty() {
        exprs.push(inventory_expression("//..."));
    }
    for label in &classified.labels {
        exprs.push(inventory_expression(label));
    }
    for pattern in &classified.patterns {
        exprs.push(inventory_expression(pattern));
    }
    for file in &classified.files {
        exprs.push(format!("tests(rdeps(//..., {}, 1))", file.label));
    }
    if ctx.is_json {
        if let Ok(event) = command_started("tests", true, "default") {
            if let Err(exit) = emit_event(ctx.out, &event) {
                return exit;
            }
        }
        if let Err(exit) = emit_event(ctx.out, &command_finished(0, &FinishedCounts::default())) {
            return exit;
        }
        return 0;
    }
    if ctx.invocation.chatty() {
        for expr in &exprs {
            if let Err(exit) = check_stdout_write(writeln!(
                ctx.out,
                "would run bazel {} --output=label_kind -- {expr}",
                ctx.verb
            )) {
                return exit;
            }
        }
        if ctx.invocation.cases {
            if let Err(exit) = check_stdout_write(writeln!(
                ctx.out,
                "would build and list cases via bazel run <label> -- --list for each supported test"
            )) {
                return exit;
            }
        }
    }
    0
}

fn emit_inventory_text(
    ctx: &mut Ctx,
    pairs: &[(String, String, String)],
    empty_scopes: &[String],
) -> i32 {
    for (_, label, _) in pairs {
        if let Err(exit) = check_stdout_write(writeln!(ctx.out, "{label}")) {
            return exit;
        }
    }
    if empty_scopes.is_empty() {
        0
    } else {
        let mut scopes = empty_scopes.to_vec();
        scopes.sort();
        scopes.dedup();
        let _ = writeln!(
            ctx.err,
            "dx: {CODE_NO_TESTS}: no tests for {}",
            scopes.join(" ")
        );
        operational_code()
    }
}

fn emit_inventory_json(
    ctx: &mut Ctx,
    pairs: &[(String, String, String)],
    empty_scopes: &[String],
    code: i32,
) -> i32 {
    if let Ok(event) = command_started("tests", false, "default") {
        if let Err(exit) = emit_event(ctx.out, &event) {
            return exit;
        }
    }
    for (_, label, scope) in pairs {
        if let Ok(event) = status_event(&StatusEvent {
            name: "tests".to_owned(),
            status: "ok".to_owned(),
            detail: label.clone(),
            hint: format!(
                "{scope} via {} ({})",
                ctx.verb,
                ctx.selection.redacted_detail()
            ),
        }) {
            if let Err(exit) = emit_event(ctx.out, &event) {
                return exit;
            }
        }
    }
    if !empty_scopes.is_empty() {
        let mut scopes = empty_scopes.to_vec();
        scopes.sort();
        scopes.dedup();
        let message = format!("no tests for {}", scopes.join(" "));
        let _ = writeln!(ctx.err, "dx: {CODE_NO_TESTS}: {message}");
        if let Ok(event) = error_event(CODE_NO_TESTS, &message, None, None, Some("query")) {
            if let Err(exit) = emit_event(ctx.out, &event) {
                return exit;
            }
        }
    }
    if let Err(exit) = emit_event(ctx.out, &command_finished(code, &FinishedCounts::default())) {
        return exit;
    }
    code
}

fn emit_cases(ctx: &mut Ctx, pairs: &[(String, String, String)], empty_scopes: &[String]) -> i32 {
    let mut listed: Vec<(String, String)> = Vec::new();
    let mut problems: Vec<(String, String)> = Vec::new();
    for (kind, label, _) in pairs {
        if !SUPPORTED_CASE_KINDS.contains(&kind.as_str()) {
            problems.push((
                CODE_UNSUPPORTED.to_owned(),
                format!("{kind} rule {label} (Rust libtest first)"),
            ));
            continue;
        }
        let argv = match list_argv(ctx, label) {
            Ok(argv) => argv,
            Err(detail) => {
                problems.push((CODE_QUERY_FAILED.to_owned(), detail));
                continue;
            }
        };
        match ctx.query_runner.run_query(&argv, ctx.workspace) {
            Ok(result) => {
                if result.code != Some(0) {
                    problems.push((
                        CODE_CASE_FAILED.to_owned(),
                        format!(
                            "bazel run {label} -- --list exited {} ({})",
                            result.code.unwrap_or(-1),
                            first_line(&result.stderr)
                        ),
                    ));
                    continue;
                }
                let mut cases = parse_cases(&result.stdout);
                if cases.is_empty() {
                    problems.push((
                        CODE_NO_CASES.to_owned(),
                        format!("{label} reported no libtest cases"),
                    ));
                    continue;
                }
                if cases.len() > MAX_CASES_PER_TARGET {
                    cases.truncate(MAX_CASES_PER_TARGET);
                    problems.push((
                        CODE_TRUNCATED.to_owned(),
                        format!("{label} truncated at {MAX_CASES_PER_TARGET} cases"),
                    ));
                }
                for case in cases {
                    listed.push((label.clone(), case));
                }
            }
            Err(error) => {
                problems.push((CODE_QUERY_FAILED.to_owned(), error.to_string()));
            }
        }
    }
    listed.sort();
    listed.dedup();
    let mut truncated_total = false;
    if listed.len() > MAX_CASES_TOTAL {
        listed.truncate(MAX_CASES_TOTAL);
        truncated_total = true;
    }
    if !empty_scopes.is_empty() {
        let mut scopes = empty_scopes.to_vec();
        scopes.sort();
        scopes.dedup();
        problems.push((
            CODE_NO_TESTS.to_owned(),
            format!("no tests for {}", scopes.join(" ")),
        ));
    }
    if truncated_total {
        problems.push((
            CODE_TRUNCATED.to_owned(),
            format!("truncated at {MAX_CASES_TOTAL} total cases"),
        ));
    }
    let code = if problems.is_empty() { 0 } else { 1 };
    if ctx.is_json {
        return emit_cases_json(ctx, &listed, &problems, code);
    }
    for (label, case) in &listed {
        if let Err(exit) = check_stdout_write(writeln!(ctx.out, "{label}: {case}")) {
            return exit;
        }
    }
    for (problem, detail) in &problems {
        let _ = writeln!(ctx.err, "dx: {problem}: {detail}");
    }
    code
}

fn emit_cases_json(
    ctx: &mut Ctx,
    listed: &[(String, String)],
    problems: &[(String, String)],
    code: i32,
) -> i32 {
    if let Ok(event) = command_started("tests", false, "default") {
        if let Err(exit) = emit_event(ctx.out, &event) {
            return exit;
        }
    }
    for (label, case) in listed {
        if let Ok(event) = status_event(&StatusEvent {
            name: "tests".to_owned(),
            status: "ok".to_owned(),
            detail: case.clone(),
            hint: format!(
                "{label} via {} ({})",
                ctx.verb,
                ctx.selection.redacted_detail()
            ),
        }) {
            if let Err(exit) = emit_event(ctx.out, &event) {
                return exit;
            }
        }
    }
    for (problem, detail) in problems {
        let _ = writeln!(ctx.err, "dx: {problem}: {detail}");
        if let Ok(event) = error_event(problem, detail, None, None, Some("cases")) {
            if let Err(exit) = emit_event(ctx.out, &event) {
                return exit;
            }
        }
    }
    if let Err(exit) = emit_event(ctx.out, &command_finished(code, &FinishedCounts::default())) {
        return exit;
    }
    code
}

#[cfg(test)]
mod tests {
    use super::super::test_support::*;
    use super::*;
    use crate::resolve::QueryResult;

    fn script_output(query: &ScriptQuery, stdout: &str) {
        query.outputs.borrow_mut().push(QueryResult {
            code: Some(0),
            stdout: stdout.as_bytes().to_vec(),
            stderr: Vec::new(),
        });
    }

    #[test]
    fn inventory_lists_sorted_unique_labels() {
        let harness = Harness::new("tests-inventory");
        script_output(
            &harness.query,
            "rust_test rule //z:two\nrust_test rule //a:one\nrust_test rule //z:two\n",
        );
        let (code, out, err) = harness.run(&["tests", "//..."]);
        assert_eq!(code, 0, "{err}");
        assert_eq!(out, "//a:one\n//z:two\n");
        assert!(err.is_empty(), "{err}");
        let calls = harness.query.calls.borrow();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0][3], "query");
        assert!(
            calls[0].contains(&"--output=label_kind".to_owned()),
            "{calls:?}"
        );
        assert_eq!(calls[0].last().expect("expr"), "tests(//...)");
    }

    #[test]
    fn inventory_parses_label_kind_and_reports_provenance() {
        let harness = Harness::new("tests-kind");
        script_output(
            &harness.query,
            "rust_test rule //a:one\npy_test rule //b:two\n",
        );
        let (code, out, _) = harness.run(&["tests", "//...", "--output=json"]);
        assert_eq!(code, 0);
        let events = json_events(&out);
        assert_eq!(event_kinds(&events)[0], "command_started");
        let details: Vec<&str> = events_of_kind(&events, "status")
            .into_iter()
            .map(|event| event["detail"].as_str().expect("detail"))
            .collect();
        assert_eq!(details, vec!["//a:one", "//b:two"]);
        for event in events_of_kind(&events, "status") {
            assert_eq!(event["name"], serde_json::json!("tests"));
            assert_eq!(event["status"], serde_json::json!("ok"));
            assert!(
                event["hint"]
                    .as_str()
                    .expect("hint")
                    .contains("unconfigured"),
                "{event}"
            );
        }
    }

    #[test]
    fn inventory_configured_uses_cquery() {
        let harness = Harness::new("tests-configured");
        script_output(&harness.query, "rust_test rule //a:one (3b48b08)\n");
        let (code, out, _) = harness.run(&["tests", "--configured", "//a:one"]);
        assert_eq!(code, 0);
        assert_eq!(out, "//a:one\n");
        let calls = harness.query.calls.borrow();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0][3], "cquery");
    }

    #[test]
    fn inventory_empty_scope_is_no_tests() {
        let harness = Harness::new("tests-empty");
        script_output(&harness.query, "\n");
        let (code, _, err) = harness.run(&["tests", "//empty/..."]);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("no_tests"), "{err}");
    }

    #[test]
    fn inventory_query_failure_is_operational() {
        let harness = Harness::new("tests-fail");
        harness.query.outputs.borrow_mut().push(QueryResult {
            code: Some(1),
            stdout: Vec::new(),
            stderr: b"no such package\n".to_vec(),
        });
        let (code, _, err) = harness.run(&["tests", "//..."]);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("bazel_failed"), "{err}");
    }

    #[test]
    fn inventory_malformed_kind_is_operational() {
        let harness = Harness::new("tests-malformed");
        script_output(&harness.query, "not a kind line\n");
        let (code, _, err) = harness.run(&["tests", "//..."]);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("bazel_failed"), "{err}");
    }

    #[test]
    fn inventory_dry_run_plans_without_query() {
        let harness = Harness::new("tests-dry");
        let (code, out, _) = harness.run(&["tests", "//a/...", "--dry-run"]);
        assert_eq!(code, 0);
        assert!(out.contains("would run bazel query"), "{out}");
        assert!(out.contains("tests(//a/...)"), "{out}");
        assert!(harness.query.calls.borrow().is_empty());
        let (code, out, _) = harness.run(&["tests", "--dry-run", "--output=json"]);
        assert_eq!(code, 0);
        let events = json_events(&out);
        assert_eq!(
            event_kinds(&events),
            vec!["command_started", "command_finished"]
        );
        assert_eq!(events[0]["dry_run"], serde_json::json!(true));
    }

    #[test]
    fn cases_dry_run_plans_enumeration_without_query() {
        let harness = Harness::new("tests-cases-dry");
        let (code, out, _) = harness.run(&["tests", "--cases", "//a/...", "--dry-run"]);
        assert_eq!(code, 0);
        assert!(out.contains("would run bazel query"), "{out}");
        assert!(out.contains("bazel run <label> -- --list"), "{out}");
        assert!(harness.query.calls.borrow().is_empty());
    }

    #[test]
    fn cases_list_rust_targets_and_mark_others_unsupported() {
        let harness = Harness::new("tests-cases");
        script_output(
            &harness.query,
            "rust_test rule //a:one\npy_test rule //b:two\n",
        );
        script_output(
            &harness.query,
            "alpha: test\nbeta: test\n1 test, 0 benchmarks\n",
        );
        let (code, out, err) = harness.run(&["tests", "--cases", "//..."]);
        assert_eq!(code, 1, "{out} {err}");
        assert_eq!(out, "//a:one: alpha\n//a:one: beta\n");
        assert!(err.contains("unsupported"), "{err}");
        assert!(err.contains("//b:two"), "{err}");
        let calls = harness.query.calls.borrow();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[1][3], "run");
        assert!(calls[1].contains(&"//a:one".to_owned()), "{calls:?}");
        assert!(calls[1].contains(&"--list".to_owned()), "{calls:?}");
    }

    #[test]
    fn cases_forwarded_rust_target_lists() {
        let harness = Harness::new("tests-forwarded");
        script_output(&harness.query, "_rust_forward_test rule //a:one\n");
        script_output(&harness.query, "solo: test\n");
        let (code, out, err) = harness.run(&["tests", "--cases", "//a:one"]);
        assert_eq!(code, 0, "{err}");
        assert_eq!(out, "//a:one: solo\n");
    }

    #[test]
    fn cases_empty_listing_is_no_cases() {
        let harness = Harness::new("tests-no-cases");
        script_output(&harness.query, "rust_test rule //a:one\n");
        script_output(&harness.query, "0 tests, 0 benchmarks\n");
        let (code, _, err) = harness.run(&["tests", "--cases", "//a:one"]);
        assert_eq!(code, 1);
        assert!(err.contains("no_cases"), "{err}");
    }

    #[test]
    fn cases_failed_run_is_operational() {
        let harness = Harness::new("tests-case-fail");
        script_output(&harness.query, "rust_test rule //a:one\n");
        harness.query.outputs.borrow_mut().push(QueryResult {
            code: Some(1),
            stdout: Vec::new(),
            stderr: b"build failed\n".to_vec(),
        });
        let (code, _, err) = harness.run(&["tests", "--cases", "//a:one"]);
        assert_eq!(code, 1);
        assert!(err.contains("case_failed"), "{err}");
    }

    #[test]
    fn cases_json_streams_per_case_and_errors() {
        let harness = Harness::new("tests-cases-json");
        script_output(
            &harness.query,
            "rust_test rule //a:one\nsh_test rule //b:two\n",
        );
        script_output(&harness.query, "alpha: test\n");
        let (code, out, _) = harness.run(&["tests", "--cases", "//...", "--output=json"]);
        assert_eq!(code, 1);
        let events = json_events(&out);
        let kinds = event_kinds(&events);
        assert_eq!(kinds[0], "command_started");
        assert_eq!(kinds[kinds.len() - 1], "command_finished");
        let details: Vec<&str> = events_of_kind(&events, "status")
            .into_iter()
            .map(|event| event["detail"].as_str().expect("detail"))
            .collect();
        assert_eq!(details, vec!["alpha"]);
        let codes: Vec<&str> = events_of_kind(&events, "error")
            .into_iter()
            .map(|event| event["code"].as_str().expect("code"))
            .collect();
        assert_eq!(codes, vec!["unsupported_framework"]);
    }

    #[test]
    fn cases_truncation_is_not_success() {
        let harness = Harness::new("tests-truncate");
        script_output(&harness.query, "rust_test rule //a:one\n");
        let mut lines = String::new();
        for index in 0..MAX_CASES_PER_TARGET + 10 {
            lines.push_str(&format!("case_{index:05}: test\n"));
        }
        script_output(&harness.query, &lines);
        let (code, out, err) = harness.run(&["tests", "--cases", "//a:one"]);
        assert_eq!(code, 1);
        assert!(err.contains("cases_truncated"), "{err}");
        assert_eq!(out.lines().count(), MAX_CASES_PER_TARGET);
    }

    #[test]
    fn file_scope_resolves_through_test_mapping() {
        let harness = Harness::new("tests-file");
        harness.write_source("pkg/BUILD.bazel", "");
        harness.write_source("pkg/a.py", "x = 1\n");
        script_output(&harness.query, "//pkg:lib\n");
        script_output(&harness.query, "//pkg:unit\n");
        script_output(&harness.query, "rust_test rule //pkg:unit\n");
        let (code, out, err) = harness.run(&["tests", "pkg/a.py"]);
        assert_eq!(code, 0, "{err}");
        assert_eq!(out, "//pkg:unit\n");
    }

    #[test]
    fn startup_options_reach_inventory_and_listing() {
        let harness = Harness::new("tests-startup");
        script_output(&harness.query, "rust_test rule //a:one\n");
        script_output(&harness.query, "solo: test\n");
        let inv = invocation(&[
            "tests",
            "--cases",
            "--bazel-startup-option=--output_base=/tmp/a",
            "//a:one",
        ]);
        let run = harness.probe_with(&inv, &[Some(0)]);
        assert_eq!(run.code, 0);
        let queries = harness.query.calls.borrow();
        assert_eq!(queries.len(), 2);
        assert_eq!(
            queries[0][..5],
            vec![
                "bazel".to_owned(),
                "--nohome_rc".to_owned(),
                "--nosystem_rc".to_owned(),
                "--output_base=/tmp/a".to_owned(),
                "query".to_owned(),
            ]
        );
        assert_eq!(
            queries[1][..5],
            vec![
                "bazel".to_owned(),
                "--nohome_rc".to_owned(),
                "--nosystem_rc".to_owned(),
                "--output_base=/tmp/a".to_owned(),
                "run".to_owned(),
            ]
        );
    }

    #[test]
    fn configured_file_scope_shares_options_across_mapping_and_inventory() {
        let harness = Harness::new("tests-configured-file");
        harness.write_source("pkg/BUILD.bazel", "");
        harness.write_source("pkg/a.py", "x = 1\n");
        script_output(&harness.query, "//pkg:lib\n");
        script_output(&harness.query, "//pkg:unit\n");
        script_output(&harness.query, "rust_test rule //pkg:unit\n");
        let (code, out, err) = harness.run(&[
            "tests",
            "--configured",
            "pkg/a.py",
            "--",
            "--platforms=//:x",
            "--config=ci",
        ]);
        assert_eq!(code, 0, "{err}");
        assert_eq!(out, "//pkg:unit\n");
        let calls = harness.query.calls.borrow();
        assert_eq!(calls.len(), 3, "{calls:?}");
        assert_eq!(
            calls[0][3], "query",
            "ownership stays unconfigured: {calls:?}"
        );
        assert_eq!(
            calls[1][3], "cquery",
            "mapping uses execution options: {calls:?}"
        );
        assert_eq!(
            calls[2][3], "cquery",
            "inventory uses execution options: {calls:?}"
        );
        for call in calls.iter().skip(1) {
            assert!(
                call.contains(&"--platforms=//:x".to_owned()),
                "mapping and inventory share platforms: {call:?}"
            );
            assert!(
                call.contains(&"--config=ci".to_owned()),
                "mapping and inventory share configs: {call:?}"
            );
        }
        assert!(
            !calls[0].contains(&"--platforms=//:x".to_owned()),
            "ownership ignores configuration: {:?}",
            calls[0]
        );
    }

    #[test]
    fn configured_inventory_hint_names_provenance_and_options() {
        let harness = Harness::new("tests-configured-hint");
        script_output(&harness.query, "rust_test rule //a:one\n");
        let (code, out, _) = harness.run(&[
            "tests",
            "--configured",
            "//a:one",
            "--output=json",
            "--",
            "--platforms=//:x",
        ]);
        assert_eq!(code, 0);
        let events = json_events(&out);
        let hints: Vec<&str> = events_of_kind(&events, "status")
            .into_iter()
            .map(|event| event["hint"].as_str().expect("hint"))
            .collect();
        assert_eq!(hints.len(), 1);
        assert!(hints[0].contains("cquery"), "{hints:?}");
        assert!(hints[0].contains("configured"), "{hints:?}");
        assert!(hints[0].contains("--platforms=//:x"), "{hints:?}");
    }

    #[test]
    fn configured_mapping_failure_is_not_empty_success() {
        let harness = Harness::new("tests-configured-fail");
        harness.write_source("pkg/BUILD.bazel", "");
        harness.write_source("pkg/a.py", "x = 1\n");
        script_output(&harness.query, "//pkg:lib\n");
        harness.query.outputs.borrow_mut().push(QueryResult {
            code: Some(6),
            stdout: Vec::new(),
            stderr: b"analysis failed\n".to_vec(),
        });
        script_output(&harness.query, "rust_test rule //pkg:unit\n");
        let (code, _, err) = harness.run(&["tests", "--configured", "pkg/a.py"]);
        assert_ne!(code, 0, "configured analysis failure must fail: {err}");
        assert!(err.contains("analysis failed"), "{err}");
        let calls = harness.query.calls.borrow();
        assert_eq!(calls.len(), 2, "{calls:?}");
        assert_eq!(calls[1][3], "cquery", "{calls:?}");
    }

    #[test]
    fn selection_drops_test_binary_args_without_rejecting() {
        let harness = Harness::new("tests-selection-args");
        harness.write_source("pkg/BUILD.bazel", "");
        harness.write_source("pkg/a.py", "x = 1\n");
        script_output(&harness.query, "//pkg:lib\n");
        script_output(&harness.query, "//pkg:unit\n");
        script_output(&harness.query, "rust_test rule //pkg:unit\n");
        let (code, out, err) = harness.run(&[
            "tests",
            "--configured",
            "pkg/a.py",
            "--",
            "--test_arg=--exact",
            "--config=ci",
        ]);
        assert_eq!(code, 0, "{err}");
        assert_eq!(out, "//pkg:unit\n");
        let calls = harness.query.calls.borrow();
        assert_eq!(calls.len(), 3, "{calls:?}");
        for call in calls.iter() {
            assert!(
                !call.iter().any(|arg| arg.contains("test_arg")),
                "selection never forwards test binary args: {call:?}"
            );
        }
        assert!(calls[1].contains(&"--config=ci".to_owned()), "{calls:?}");
    }

    #[test]
    fn selection_hint_withholds_secrets() {
        let harness = Harness::new("tests-selection-secret");
        script_output(&harness.query, "rust_test rule //a:one\n");
        let (code, out, _) = harness.run(&[
            "tests",
            "--configured",
            "//a:one",
            "--output=json",
            "--",
            "--token=abc",
        ]);
        assert_eq!(code, 0);
        let events = json_events(&out);
        let hints: Vec<&str> = events_of_kind(&events, "status")
            .into_iter()
            .map(|event| event["hint"].as_str().expect("hint"))
            .collect();
        assert_eq!(hints.len(), 1);
        assert!(hints[0].contains("--token"), "{hints:?}");
        assert!(!hints[0].contains("abc"), "{hints:?}");
        let calls = harness.query.calls.borrow();
        assert!(calls[0].contains(&"--token=abc".to_owned()), "{calls:?}");
    }
}
