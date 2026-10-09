use std::io::Write;

use crate::args::{Command, Invocation};
use crate::exec::common::{check_stdout_write, emit_event};
use crate::resolve::QueryRunner;
use dx_output::{
    command_finished, command_started, error_event, status_event, FinishedCounts, OutputMode,
    StatusEvent,
};
use dx_process::operational_code;

use super::{operational, pre_exec, summaries_suppressed};

pub(crate) const CODE_QUERY_FAILED: &str = "bazel_failed";
pub(crate) const CODE_NO_OWNER: &str = "no_owner";
pub(crate) const CODE_WHY_INVALID: &str = "invalid_result";

fn inspect_argv(invocation: &Invocation, verb: &str, expr: &str) -> Vec<String> {
    let mut argv = dx_process::startup_argv(&invocation.bazel_startup_options);
    argv.push(verb.to_owned());
    argv.push(expr.to_owned());
    argv
}

pub(crate) fn execute_inspect(
    invocation: &Invocation,
    workspace: &std::path::Path,
    query_runner: &dyn QueryRunner,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    if invocation.command == Command::Why {
        return execute_why(invocation, workspace, query_runner, out, err);
    }
    let kind = invocation.command.name();
    let is_json = invocation.output == OutputMode::Json;
    if invocation.dry_run {
        for scope in &invocation.targets {
            if let Err(error) = dx_adopt::plan_inspect(kind, scope, invocation.configured) {
                return pre_exec(err, &error.to_string());
            }
        }
        if is_json {
            if let Ok(event) = command_started(kind, true, "default") {
                if let Err(exit) = emit_event(out, &event) {
                    return exit;
                }
            }
            if let Err(exit) = emit_event(out, &command_finished(0, &FinishedCounts::default())) {
                return exit;
            }
            return 0;
        }
        if !summaries_suppressed(invocation) {
            for scope in &invocation.targets {
                if let Ok(plan) = dx_adopt::plan_inspect(kind, scope, invocation.configured) {
                    if let Err(exit) = check_stdout_write(writeln!(
                        out,
                        "would run bazel {} {}",
                        plan.verb, plan.expr
                    )) {
                        return exit;
                    }
                }
            }
        }
        return 0;
    }
    if is_json {
        for scope in &invocation.targets {
            if let Err(error) = dx_adopt::plan_inspect(kind, scope, invocation.configured) {
                return pre_exec(err, &error.to_string());
            }
        }
        if let Ok(event) = command_started(kind, false, "default") {
            if let Err(exit) = emit_event(out, &event) {
                return exit;
            }
        }
        let mut failed = false;
        for scope in &invocation.targets {
            let plan = match dx_adopt::plan_inspect(kind, scope, invocation.configured) {
                Ok(plan) => plan,
                Err(error) => return pre_exec(err, &error.to_string()),
            };
            if let Err(exit) =
                run_inspect_query_json(invocation, scope, &plan, workspace, query_runner, out, err)
            {
                if exit == operational_code() {
                    failed = true;
                    continue;
                }
                return exit;
            }
        }
        let code = if failed { operational_code() } else { 0 };
        if let Err(exit) = emit_event(out, &command_finished(code, &FinishedCounts::default())) {
            return exit;
        }
        return code;
    }
    let mut code = 0;
    for scope in &invocation.targets {
        let plan = match dx_adopt::plan_inspect(kind, scope, invocation.configured) {
            Ok(plan) => plan,
            Err(error) => return pre_exec(err, &error.to_string()),
        };
        let step = run_inspect_query(
            invocation,
            &plan.verb,
            &plan.expr,
            workspace,
            query_runner,
            out,
            err,
        );
        if step != 0 {
            code = step;
        }
    }
    code
}

fn run_inspect_query_json(
    invocation: &Invocation,
    scope: &str,
    plan: &dx_adopt::InspectPlan,
    workspace: &std::path::Path,
    query_runner: &dyn QueryRunner,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<(), i32> {
    let kind = invocation.command.name();
    let argv = inspect_argv(invocation, &plan.verb, &plan.expr);
    match query_runner.run_query(&argv, workspace) {
        Ok(result) => {
            if result.code != Some(0) {
                let message = format!(
                    "query failed: bazel {} {} exited with code {}",
                    plan.verb,
                    plan.expr,
                    result.code.unwrap_or(-1)
                );
                let _ = writeln!(err, "dx: {message}");
                if let Ok(event) =
                    error_event(CODE_QUERY_FAILED, &message, None, None, Some("query"))
                {
                    emit_event(out, &event)?;
                }
                return Err(operational_code());
            }
            let text = String::from_utf8_lossy(&result.stdout);
            let mut lines: Vec<&str> = text.lines().collect();
            lines.sort_unstable();
            lines.dedup();
            for line in lines {
                if let Ok(event) = status_event(&StatusEvent {
                    name: kind.to_owned(),
                    status: "ok".to_owned(),
                    detail: line.to_owned(),
                    hint: scope.to_owned(),
                }) {
                    emit_event(out, &event)?;
                }
            }
            Ok(())
        }
        Err(error) => {
            let message = error.to_string();
            let _ = writeln!(err, "dx: {message}");
            if let Ok(event) = error_event(CODE_QUERY_FAILED, &message, None, None, Some("query")) {
                emit_event(out, &event)?;
            }
            Err(operational_code())
        }
    }
}

fn run_inspect_query(
    invocation: &Invocation,
    verb: &str,
    expr: &str,
    workspace: &std::path::Path,
    query_runner: &dyn QueryRunner,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let argv = inspect_argv(invocation, verb, expr);
    match query_runner.run_query(&argv, workspace) {
        Ok(result) => {
            if result.code != Some(0) {
                return operational(
                    invocation,
                    out,
                    err,
                    CODE_QUERY_FAILED,
                    &format!(
                        "query failed: bazel {verb} {expr} exited with code {}",
                        result.code.unwrap_or(-1)
                    ),
                );
            }
            let text = String::from_utf8_lossy(&result.stdout);
            let mut lines: Vec<&str> = text.lines().collect();
            lines.sort_unstable();
            lines.dedup();
            for line in lines {
                if let Err(exit) = check_stdout_write(writeln!(out, "{line}")) {
                    return exit;
                }
            }
            0
        }
        Err(error) => operational(invocation, out, err, CODE_QUERY_FAILED, &error.to_string()),
    }
}

fn execute_why(
    invocation: &Invocation,
    workspace: &std::path::Path,
    query_runner: &dyn QueryRunner,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let Some(file) = invocation.targets.first() else {
        return pre_exec(err, "why needs exactly <file> <label>");
    };
    let Some(label) = invocation.targets.get(1) else {
        return pre_exec(err, "why needs exactly <file> <label>");
    };
    if invocation.targets.len() != 2 {
        return pre_exec(err, "why needs exactly <file> <label>");
    }
    let is_json = invocation.output == OutputMode::Json;
    if invocation.dry_run {
        let owner_plan = match dx_adopt::plan_inspect("owners", file, invocation.configured) {
            Ok(plan) => plan,
            Err(error) => return pre_exec(err, &error.to_string()),
        };
        if is_json {
            if let Ok(event) = command_started("why", true, "default") {
                if let Err(exit) = emit_event(out, &event) {
                    return exit;
                }
            }
            if let Err(exit) = emit_event(out, &command_finished(0, &FinishedCounts::default())) {
                return exit;
            }
            return 0;
        }
        if !summaries_suppressed(invocation) {
            if let Err(exit) = check_stdout_write(writeln!(
                out,
                "would run bazel {} {} then somepath to {label}",
                owner_plan.verb, owner_plan.expr
            )) {
                return exit;
            }
        }
        return 0;
    }
    if is_json {
        return execute_why_json(invocation, file, label, workspace, query_runner, out, err);
    }
    let owner_plan = match dx_adopt::plan_inspect("owners", file, invocation.configured) {
        Ok(plan) => plan,
        Err(error) => return pre_exec(err, &error.to_string()),
    };
    let owner_argv = inspect_argv(invocation, &owner_plan.verb, &owner_plan.expr);
    let owner = match query_runner.run_query(&owner_argv, workspace) {
        Ok(result) => {
            if result.code != Some(0) {
                return operational(
                    invocation,
                    out,
                    err,
                    CODE_QUERY_FAILED,
                    &format!(
                        "query failed: bazel {} {} for file {file} exited with code {}",
                        owner_plan.verb,
                        owner_plan.expr,
                        result.code.unwrap_or(-1)
                    ),
                );
            }
            let text = String::from_utf8_lossy(&result.stdout);
            let mut labels: Vec<&str> = text.lines().collect();
            labels.sort_unstable();
            labels.dedup();
            match labels.into_iter().next() {
                Some(owner) => owner.to_owned(),
                None => {
                    return operational(
                        invocation,
                        out,
                        err,
                        CODE_NO_OWNER,
                        &format!(
                            "no owner for {file} via bazel {} {}",
                            owner_plan.verb, owner_plan.expr
                        ),
                    );
                }
            }
        }
        Err(error) => {
            return operational(invocation, out, err, CODE_QUERY_FAILED, &error.to_string());
        }
    };
    let leg = match dx_adopt::plan_somepath(&owner, label, invocation.configured) {
        Ok(leg) => leg,
        Err(error) => return pre_exec(err, &error.to_string()),
    };
    run_inspect_query(
        invocation,
        &leg.verb,
        &leg.expr,
        workspace,
        query_runner,
        out,
        err,
    )
}

fn execute_why_json(
    invocation: &Invocation,
    file: &str,
    label: &str,
    workspace: &std::path::Path,
    query_runner: &dyn QueryRunner,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    if let Err(error) = dx_adopt::plan_inspect("owners", file, invocation.configured) {
        return pre_exec(err, &error.to_string());
    }
    if file.is_empty() || label.is_empty() || label.starts_with('@') || file.starts_with('@') {
        return pre_exec(err, "why needs exactly <file> <label>");
    }
    if let Ok(event) = command_started("why", false, "default") {
        if let Err(exit) = emit_event(out, &event) {
            return exit;
        }
    }
    let owner_plan = match dx_adopt::plan_inspect("owners", file, invocation.configured) {
        Ok(plan) => plan,
        Err(error) => return pre_exec(err, &error.to_string()),
    };
    let owner_argv = inspect_argv(invocation, &owner_plan.verb, &owner_plan.expr);
    let owner = match query_runner.run_query(&owner_argv, workspace) {
        Ok(result) => {
            if result.code != Some(0) {
                let message = format!(
                    "query failed: bazel {} {} for file {file} exited with code {}",
                    owner_plan.verb,
                    owner_plan.expr,
                    result.code.unwrap_or(-1)
                );
                let _ = writeln!(err, "dx: {message}");
                if let Ok(event) =
                    error_event(CODE_QUERY_FAILED, &message, None, None, Some("query"))
                {
                    if let Err(exit) = emit_event(out, &event) {
                        return exit;
                    }
                }
                if let Err(exit) = emit_event(
                    out,
                    &command_finished(operational_code(), &FinishedCounts::default()),
                ) {
                    return exit;
                }
                return operational_code();
            }
            let text = String::from_utf8_lossy(&result.stdout);
            let mut labels: Vec<&str> = text.lines().collect();
            labels.sort_unstable();
            labels.dedup();
            match labels.into_iter().next() {
                Some(owner) => owner.to_owned(),
                None => {
                    let message = format!(
                        "no owner for {file} via bazel {} {}",
                        owner_plan.verb, owner_plan.expr
                    );
                    let _ = writeln!(err, "dx: {message}");
                    if let Ok(event) = error_event(CODE_NO_OWNER, &message, None, None, None) {
                        if let Err(exit) = emit_event(out, &event) {
                            return exit;
                        }
                    }
                    if let Err(exit) = emit_event(
                        out,
                        &command_finished(operational_code(), &FinishedCounts::default()),
                    ) {
                        return exit;
                    }
                    return operational_code();
                }
            }
        }
        Err(error) => {
            let message = error.to_string();
            let _ = writeln!(err, "dx: {message}");
            if let Ok(event) = error_event(CODE_QUERY_FAILED, &message, None, None, Some("query")) {
                if let Err(exit) = emit_event(out, &event) {
                    return exit;
                }
            }
            if let Err(exit) = emit_event(
                out,
                &command_finished(operational_code(), &FinishedCounts::default()),
            ) {
                return exit;
            }
            return operational_code();
        }
    };
    let leg = match dx_adopt::plan_somepath(&owner, label, invocation.configured) {
        Ok(leg) => leg,
        Err(error) => {
            let message = error.to_string();
            let _ = writeln!(err, "dx: {message}");
            if let Ok(event) = error_event(CODE_WHY_INVALID, &message, None, None, None) {
                if let Err(exit) = emit_event(out, &event) {
                    return exit;
                }
            }
            if let Err(exit) = emit_event(
                out,
                &command_finished(operational_code(), &FinishedCounts::default()),
            ) {
                return exit;
            }
            return operational_code();
        }
    };
    let argv = inspect_argv(invocation, &leg.verb, &leg.expr);
    match query_runner.run_query(&argv, workspace) {
        Ok(result) => {
            if result.code != Some(0) {
                let message = format!(
                    "query failed: bazel {} {} exited with code {}",
                    leg.verb,
                    leg.expr,
                    result.code.unwrap_or(-1)
                );
                let _ = writeln!(err, "dx: {message}");
                if let Ok(event) =
                    error_event(CODE_QUERY_FAILED, &message, None, None, Some("query"))
                {
                    if let Err(exit) = emit_event(out, &event) {
                        return exit;
                    }
                }
                if let Err(exit) = emit_event(
                    out,
                    &command_finished(operational_code(), &FinishedCounts::default()),
                ) {
                    return exit;
                }
                return operational_code();
            }
            let text = String::from_utf8_lossy(&result.stdout);
            let mut lines: Vec<&str> = text.lines().collect();
            lines.sort_unstable();
            lines.dedup();
            let hint = format!("{file} -> {label}");
            for line in lines {
                if let Ok(event) = status_event(&StatusEvent {
                    name: "why".to_owned(),
                    status: "ok".to_owned(),
                    detail: line.to_owned(),
                    hint: hint.clone(),
                }) {
                    if let Err(exit) = emit_event(out, &event) {
                        return exit;
                    }
                }
            }
            if let Err(exit) = emit_event(out, &command_finished(0, &FinishedCounts::default())) {
                return exit;
            }
            0
        }
        Err(error) => {
            let message = error.to_string();
            let _ = writeln!(err, "dx: {message}");
            if let Ok(event) = error_event(CODE_QUERY_FAILED, &message, None, None, Some("query")) {
                if let Err(exit) = emit_event(out, &event) {
                    return exit;
                }
            }
            if let Err(exit) = emit_event(
                out,
                &command_finished(operational_code(), &FinishedCounts::default()),
            ) {
                return exit;
            }
            operational_code()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adopt::test_support::{
        event_kinds, events_of_kind, invocation, json_events, run_with_query, Truncated,
    };
    use crate::resolve::QueryResult;
    use std::io;

    struct FailingLeg {
        calls: std::cell::Cell<usize>,
        leg: usize,
        spawn_error: bool,
    }

    impl QueryRunner for FailingLeg {
        fn run_query(&self, _: &[String], _: &std::path::Path) -> io::Result<QueryResult> {
            let call = self.calls.get();
            self.calls.set(call + 1);
            if call == self.leg {
                if self.spawn_error {
                    return Err(io::Error::new(io::ErrorKind::NotFound, "bazel unavailable"));
                }
                return Ok(QueryResult {
                    code: None,
                    stdout: Vec::new(),
                    stderr: Vec::new(),
                });
            }
            Ok(QueryResult {
                code: Some(0),
                stdout: b"//owner:lib\n".to_vec(),
                stderr: Vec::new(),
            })
        }
    }

    #[test]
    fn inspect_query_failures_preserve_lifecycle_and_stop_why() {
        for words in [
            vec!["owners", "src/lib.rs"],
            vec!["why", "src/lib.rs", "//app:server"],
        ] {
            for json in [false, true] {
                for spawn_error in [false, true] {
                    for leg in 0..if words[0] == "why" { 2 } else { 1 } {
                        let mut inv = invocation(&words);
                        if json {
                            inv.output = OutputMode::Json;
                        }
                        let query = FailingLeg {
                            calls: std::cell::Cell::new(0),
                            leg,
                            spawn_error,
                        };
                        let mut out = Vec::new();
                        let mut err = Vec::new();
                        assert_eq!(
                            execute_inspect(
                                &inv,
                                std::path::Path::new("."),
                                &query,
                                &mut out,
                                &mut err
                            ),
                            1
                        );
                        assert_eq!(query.calls.get(), leg + 1);
                        assert!(!err.is_empty());
                        if json {
                            let events: Vec<serde_json::Value> = String::from_utf8(out)
                                .expect("stdout")
                                .lines()
                                .map(|line| serde_json::from_str(line).expect("event"))
                                .collect();
                            assert_eq!(events[0]["event"], "command_started");
                            assert_eq!(events[1]["code"], "bazel_failed");
                            assert_eq!(events.last().expect("finished")["exit_code"], 1);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn inspect_broken_pipe_stops_at_each_output_boundary() {
        for words in [
            vec!["owners", "src/lib.rs"],
            vec!["why", "src/lib.rs", "//app:server"],
        ] {
            for json in [false, true] {
                for dry_run in [false, true] {
                    let mut inv = invocation(&words);
                    inv.dry_run = dry_run;
                    if json {
                        inv.output = OutputMode::Json;
                    }
                    let lines = if json {
                        if dry_run {
                            2
                        } else {
                            3
                        }
                    } else {
                        1
                    };
                    for remaining_lines in 0..lines {
                        let query = ScriptedQuery::with(&["//owner:lib\n", "//app:server\n"]);
                        let mut out = Truncated::after_lines(remaining_lines);
                        assert_eq!(
                            execute_inspect(
                                &inv,
                                std::path::Path::new("."),
                                &query,
                                &mut out,
                                &mut Vec::new()
                            ),
                            141,
                            "{words:?} json={json} dry={dry_run}"
                        );
                    }
                }
                if json {
                    for spawn_error in [false, true] {
                        for leg in 0..if words[0] == "why" { 2 } else { 1 } {
                            for remaining_lines in 1..3 {
                                let mut inv = invocation(&words);
                                inv.output = OutputMode::Json;
                                let query = FailingLeg {
                                    calls: std::cell::Cell::new(0),
                                    leg,
                                    spawn_error,
                                };
                                assert_eq!(
                                    execute_inspect(
                                        &inv,
                                        std::path::Path::new("."),
                                        &query,
                                        &mut Truncated::after_lines(remaining_lines),
                                        &mut Vec::new()
                                    ),
                                    141
                                );
                            }
                        }
                    }
                    for owner in ["", "@external//:owner\n"] {
                        for remaining_lines in 1..3 {
                            let mut inv = invocation(&["why", "src/lib.rs", "//app:server"]);
                            inv.output = OutputMode::Json;
                            assert_eq!(
                                execute_inspect(
                                    &inv,
                                    std::path::Path::new("."),
                                    &ScriptedQuery::with(&[owner]),
                                    &mut Truncated::after_lines(remaining_lines),
                                    &mut Vec::new()
                                ),
                                141
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn inspect_rejects_hand_built_external_scopes_before_query() {
        for command in ["owners", "deps", "why"] {
            for json in [false, true] {
                for dry_run in [false, true] {
                    let mut inv = if command == "why" {
                        invocation(&[command, "src/lib.rs", "//app:server"])
                    } else {
                        invocation(&[command, "src/lib.rs"])
                    };
                    inv.targets[0] = "@external//:lib".to_owned();
                    inv.dry_run = dry_run;
                    if json {
                        inv.output = OutputMode::Json;
                    }
                    let query = ScriptedQuery::with(&[]);
                    let mut out = Vec::new();
                    assert_eq!(
                        execute_inspect(
                            &inv,
                            std::path::Path::new("."),
                            &query,
                            &mut out,
                            &mut Vec::new()
                        ),
                        2
                    );
                    assert!(query.calls.borrow().is_empty());
                    assert!(out.is_empty());
                }
            }
        }
    }

    struct ScriptedQuery {
        calls: std::cell::RefCell<Vec<Vec<String>>>,
        outputs: Vec<Vec<u8>>,
        code: Option<i32>,
    }

    impl ScriptedQuery {
        fn with(outputs: &[&str]) -> Self {
            Self {
                calls: std::cell::RefCell::new(Vec::new()),
                outputs: outputs
                    .iter()
                    .map(|text| text.as_bytes().to_vec())
                    .collect(),
                code: Some(0),
            }
        }
    }

    impl QueryRunner for ScriptedQuery {
        fn run_query(&self, argv: &[String], _cwd: &std::path::Path) -> io::Result<QueryResult> {
            let mut calls = self.calls.borrow_mut();
            let stdout = self.outputs.get(calls.len()).cloned().unwrap_or_default();
            calls.push(argv.to_vec());
            Ok(QueryResult {
                code: self.code,
                stdout,
                stderr: Vec::new(),
            })
        }
    }

    #[test]
    fn inspect_forwards_single_unwrapped_query() {
        let runner = ScriptedQuery::with(&["//z:two\n//a:one\n//z:two\n"]);
        let inv = invocation(&["owners", "//a:one"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-inspect-single-");
        let root = scratch.path().to_path_buf();
        let (code, out, err) = run_with_query(&inv, &root, &runner);
        assert_eq!(code, 0);
        assert_eq!(out, "//a:one\n//z:two\n");
        let calls = runner.calls.borrow();
        assert_eq!(calls.len(), 1);
        assert_eq!(
            calls[0],
            vec![
                "bazel".to_owned(),
                "--nohome_rc".to_owned(),
                "--nosystem_rc".to_owned(),
                "query".to_owned(),
                "kind('rule', rdeps(//..., //a:one, 1))".to_owned(),
            ]
        );
        assert!(err.is_empty());
    }

    #[test]
    fn inspect_configured_uses_cquery() {
        let runner = ScriptedQuery::with(&["//a:one\n"]);
        let inv = invocation(&["deps", "--configured", "//a:one"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-inspect-configured-");
        let root = scratch.path().to_path_buf();
        let (code, _out, _err) = run_with_query(&inv, &root, &runner);
        assert_eq!(code, 0);
        let calls = runner.calls.borrow();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0][3], "cquery");
        assert_eq!(calls[0][4], "deps(//a:one)");
    }

    #[test]
    fn inspect_carries_startup_options_before_the_verb() {
        let runner = ScriptedQuery::with(&["//a:one\n"]);
        let inv = invocation(&[
            "owners",
            "--bazel-startup-option=--output_base=/tmp/a",
            "//a:one",
        ]);
        let scratch = dx_test_scratch::scratch("dx-adopt-inspect-startup-");
        let root = scratch.path().to_path_buf();
        let (code, _out, err) = run_with_query(&inv, &root, &runner);
        assert_eq!(code, 0, "{err}");
        let calls = runner.calls.borrow();
        assert_eq!(calls.len(), 1);
        assert_eq!(
            calls[0],
            vec![
                "bazel".to_owned(),
                "--nohome_rc".to_owned(),
                "--nosystem_rc".to_owned(),
                "--output_base=/tmp/a".to_owned(),
                "query".to_owned(),
                "kind('rule', rdeps(//..., //a:one, 1))".to_owned(),
            ]
        );
    }

    #[test]
    fn why_resolves_owner_then_somepath() {
        let runner = ScriptedQuery::with(&["//owner:lib\n", "//owner:lib\n//app:server\n"]);
        let inv = invocation(&["why", "src/lib.rs", "//app:server"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-inspect-why-");
        let root = scratch.path().to_path_buf();
        let (code, out, _err) = run_with_query(&inv, &root, &runner);
        assert_eq!(code, 0);
        let calls = runner.calls.borrow();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0][4], "kind('rule', rdeps(//..., src/lib.rs, 1))");
        assert_eq!(
            calls[1],
            vec![
                "bazel".to_owned(),
                "--nohome_rc".to_owned(),
                "--nosystem_rc".to_owned(),
                "query".to_owned(),
                "somepath(//owner:lib, //app:server)".to_owned(),
            ]
        );
        assert!(out.contains("//app:server"));
    }

    #[test]
    fn why_without_owner_is_operational() {
        let runner = ScriptedQuery::with(&[""]);
        let inv = invocation(&["why", "src/orphan.rs", "//app:server"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-inspect-why-orphan-");
        let root = scratch.path().to_path_buf();
        let (code, _out, err) = run_with_query(&inv, &root, &runner);
        assert_eq!(code, 1);
        assert!(err.contains("no owner"));
        assert_eq!(runner.calls.borrow().len(), 1);
    }

    #[test]
    fn inspect_dry_run_plans_without_query() {
        let runner = ScriptedQuery::with(&["//a:one\n"]);
        let inv = invocation(&["owners", "//a:one", "--dry-run"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-inspect-dry-");
        let root = scratch.path().to_path_buf();
        let (code, out, _err) = run_with_query(&inv, &root, &runner);
        assert_eq!(code, 0);
        assert!(out.contains("would run bazel"));
        assert_eq!(runner.calls.borrow().len(), 0);
        let runner = ScriptedQuery::with(&["//owner:lib\n"]);
        let inv = invocation(&["why", "src/lib.rs", "//app:server", "--dry-run"]);
        let (code, out, _err) = run_with_query(&inv, &root, &runner);
        assert_eq!(code, 0);
        assert!(out.contains("would run bazel"));
        assert_eq!(runner.calls.borrow().len(), 0);
    }

    #[test]
    fn why_malformed_invocation_fails_closed_without_panic() {
        let base = invocation(&["why", "src/lib.rs", "//app:server"]);
        for targets in [
            Vec::new(),
            vec!["only-one".to_owned()],
            vec![
                "src/lib.rs".to_owned(),
                "//app:server".to_owned(),
                "//extra:lib".to_owned(),
            ],
        ] {
            let mut bad = base.clone();
            bad.targets = targets;
            let runner = ScriptedQuery::with(&["//owner:lib\n"]);
            let scratch = dx_test_scratch::scratch("dx-adopt-inspect-why-malformed-");
            let root = scratch.path().to_path_buf();
            let (code, _out, err) = run_with_query(&bad, &root, &runner);
            assert_eq!(code, 2, "targets: {:?}", bad.targets);
            assert!(
                err.contains("why needs exactly"),
                "targets: {:?}",
                bad.targets
            );
            assert_eq!(runner.calls.borrow().len(), 0);
        }
    }

    #[test]
    fn inspect_json_streams_status_per_label() {
        let runner = ScriptedQuery::with(&["//z:two\n//a:one\n//z:two\n"]);
        let inv = invocation(&["owners", "//a:one", "--output=json"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-inspect-json-");
        let root = scratch.path().to_path_buf();
        let (code, out, err) = run_with_query(&inv, &root, &runner);
        assert_eq!(code, 0);
        let text = out;
        let events = json_events(&text);
        let kinds = event_kinds(&events);
        assert_eq!(kinds[0], "command_started");
        assert_eq!(kinds[kinds.len() - 1], "command_finished");
        assert!(!kinds.contains(&"error"), "{kinds:?}");
        assert_eq!(
            events.last().expect("finished")["exit_code"],
            serde_json::json!(0)
        );
        let details: Vec<&str> = events_of_kind(&events, "status")
            .into_iter()
            .map(|event| event["detail"].as_str().expect("detail"))
            .collect();
        assert_eq!(details, vec!["//a:one", "//z:two"], "{details:?}");
        for event in events_of_kind(&events, "status") {
            assert_eq!(event["name"], serde_json::json!("owners"), "{event}");
            assert_eq!(event["status"], serde_json::json!("ok"), "{event}");
            assert_eq!(event["hint"], serde_json::json!("//a:one"), "{event}");
        }
        assert!(err.is_empty());
    }

    #[test]
    fn inspect_query_failure_json_emits_bazel_failed() {
        struct FailingQuery;
        impl QueryRunner for FailingQuery {
            fn run_query(
                &self,
                _argv: &[String],
                _cwd: &std::path::Path,
            ) -> io::Result<QueryResult> {
                Ok(QueryResult {
                    code: Some(1),
                    stdout: Vec::new(),
                    stderr: Vec::new(),
                })
            }
        }
        let inv = invocation(&["deps", "//a:one", "--output=json"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-inspect-json-fail-");
        let root = scratch.path().to_path_buf();
        let (code, text, _err) = run_with_query(&inv, &root, &FailingQuery);
        assert_eq!(code, 1);
        let events = json_events(&text);
        let kinds = event_kinds(&events);
        assert_eq!(
            kinds,
            vec!["command_started", "error", "command_finished"],
            "{kinds:?}"
        );
        assert_eq!(events[1]["code"], serde_json::json!("bazel_failed"));
        assert_eq!(events[1]["phase"], serde_json::json!("query"));
    }

    #[test]
    fn inspect_dry_run_json_emits_lifecycle_only() {
        for words in [
            vec!["owners", "//a:one", "--dry-run", "--output=json"],
            vec![
                "why",
                "src/lib.rs",
                "//app:server",
                "--dry-run",
                "--output=json",
            ],
        ] {
            let runner = ScriptedQuery::with(&["//a:one\n"]);
            let inv = invocation(&words);
            let scratch = dx_test_scratch::scratch("dx-adopt-inspect-dry-json-");
            let root = scratch.path().to_path_buf();
            let (code, out, _err) = run_with_query(&inv, &root, &runner);
            assert_eq!(code, 0, "words: {words:?}");
            let text = out;
            let events = json_events(&text);
            let kinds = event_kinds(&events);
            assert_eq!(
                kinds,
                vec!["command_started", "command_finished"],
                "words: {words:?}"
            );
            assert_eq!(events[0]["dry_run"], serde_json::json!(true));
            assert_eq!(runner.calls.borrow().len(), 0, "words: {words:?}");
        }
    }

    #[test]
    fn why_json_streams_somepath_labels() {
        let runner = ScriptedQuery::with(&["//owner:lib\n", "//owner:lib\n//app:server\n"]);
        let inv = invocation(&["why", "src/lib.rs", "//app:server", "--output=json"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-inspect-why-json-");
        let root = scratch.path().to_path_buf();
        let (code, out, _err) = run_with_query(&inv, &root, &runner);
        assert_eq!(code, 0);
        let text = out;
        let events = json_events(&text);
        let kinds = event_kinds(&events);
        assert_eq!(kinds[0], "command_started");
        assert_eq!(kinds[kinds.len() - 1], "command_finished");
        assert!(!kinds.contains(&"error"), "{kinds:?}");
        for event in events_of_kind(&events, "status") {
            assert_eq!(event["name"], serde_json::json!("why"), "{event}");
            assert_eq!(
                event["hint"],
                serde_json::json!("src/lib.rs -> //app:server"),
                "{event}"
            );
        }
    }

    #[test]
    fn why_without_owner_json_emits_no_owner() {
        let runner = ScriptedQuery::with(&[""]);
        let inv = invocation(&["why", "src/orphan.rs", "//app:server", "--output=json"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-inspect-why-json-orphan-");
        let root = scratch.path().to_path_buf();
        let (code, out, _err) = run_with_query(&inv, &root, &runner);
        assert_eq!(code, 1);
        let text = out;
        let events = json_events(&text);
        let kinds = event_kinds(&events);
        assert_eq!(
            kinds,
            vec!["command_started", "error", "command_finished"],
            "{kinds:?}"
        );
        assert_eq!(events[1]["code"], serde_json::json!("no_owner"));
    }
}
