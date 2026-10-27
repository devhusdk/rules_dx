use super::common::*;
use dx_cli_core::args::Invocation;
use dx_cli_core::resolve::{first_line, quote_set, resolve, QueryRunner, SelectionContext};
use dx_output::{
    command_finished, command_started, error_event, operation_event, FinishedCounts, OutputMode,
};
use std::path::{Path, PathBuf};

const DOCS_CHECK_TARGET: &str = "//tools/docs/site:user_site_check";
const DOCS_BUILD_TARGET: &str = "//tools/docs/site:user_site";

const DOCS_DEFAULT_PORT: u16 = 8000;

const DOCS_DEFAULT_HOST: &str = "127.0.0.1";

pub(crate) const CODE_SERVE_FAILED: &str = "serve_failed";

fn preview_url(host: &str, port: u16) -> String {
    format!("http://{host}:{port}/")
}

fn opener_argv(url: &str) -> Vec<String> {
    vec![
        "python3".to_owned(),
        "-c".to_owned(),
        "import sys, webbrowser; webbrowser.open(sys.argv[1])".to_owned(),
        url.to_owned(),
    ]
}

fn site_tree_expression(labels: &[String]) -> String {
    if labels.len() == 1 {
        labels[0].clone()
    } else {
        format!("set({})", quote_set(labels))
    }
}

fn resolve_site_dir(
    labels: &[String],
    scope_text: &str,
    workspace: &Path,
    query_runner: &dyn QueryRunner,
    startup_options: &[String],
) -> Result<PathBuf, String> {
    let expression = site_tree_expression(labels);
    let argv = SelectionContext::unconfigured(startup_options)
        .selection_argv("cquery", &["--output=files".to_owned()], &expression)
        .map_err(|error| {
            format!("could not scope the built site query for {scope_text}: {error}")
        })?;
    let result = query_runner.run_query(&argv, workspace).map_err(|error| {
        format!("could not run cquery for the built site ({scope_text}): {error}")
    })?;
    if result.code != Some(0) {
        return Err(format!(
            "cquery for the built site ({scope_text}) failed: {}",
            first_line(&result.stderr)
        ));
    }
    let text = std::str::from_utf8(&result.stdout).map_err(|_| {
        format!("cquery for the built site ({scope_text}) returned non-UTF-8 output")
    })?;
    let paths: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    let [path] = paths.as_slice() else {
        return Err(format!(
            "cquery for the built site ({scope_text}) reported {} output files; serve needs exactly one built site tree (pass a single site target)",
            paths.len()
        ));
    };
    let dir = workspace.join(path);
    if !dir.join("index.html").is_file() {
        return Err(format!(
            "the built site for {scope_text} has no index.html entry page at {}",
            dir.display()
        ));
    }
    Ok(dir)
}

pub(crate) fn execute_docs(invocation: &Invocation, env: Env<'_>) -> i32 {
    let Env {
        workspace,
        runner,
        query_runner,
        out,
        err,
        ..
    } = env;
    let json = invocation.output == OutputMode::Json;
    let mode = if invocation.check { "check" } else { "default" };
    let (labels, scope_text) = if invocation.targets.is_empty() {
        let target = if invocation.check {
            DOCS_CHECK_TARGET
        } else {
            DOCS_BUILD_TARGET
        };
        (vec![target.to_owned()], target.to_owned())
    } else {
        match resolve(
            &invocation.targets,
            workspace,
            query_runner,
            &invocation.bazel_startup_options,
        ) {
            Ok(resolved) => {
                let scope_text = if resolved.targets.is_empty() {
                    "//...".to_owned()
                } else {
                    resolved.targets.join(" ")
                };
                (resolved.targets, scope_text)
            }
            Err(error) => return pre_exec(invocation.output, out, err, &error.to_string()),
        }
    };
    let (actions, summary): (&[&str], String) = if invocation.check {
        (
            &["summary"],
            format!("Running docs check for {scope_text} (no render)"),
        )
    } else {
        (
            &["summary", "render"],
            format!("Running docs build for {scope_text} (mdBook render)"),
        )
    };
    let bazel_options = invocation.quiet_bazel_options();
    let argv = match dx_cli_core::plan::bazel_argv(
        "build",
        &bazel_options,
        &[],
        &[],
        &labels,
        &invocation.bazel_startup_options,
    ) {
        Ok(argv) => argv,
        Err(error) => return pre_exec(invocation.output, out, err, &format!("{error}")),
    };
    if json {
        if let Err(exit) = emit_built(out, command_started(invocation.command.name(), mode)) {
            return exit;
        }
        for action in actions {
            if let Err(exit) = emit_built(
                out,
                operation_event(invocation.command.name(), action, Some(&labels)),
            ) {
                return exit;
            }
        }
    }
    if !json && invocation.chatty() {
        if let Err(exit) = check_stdout_write(writeln!(out, "{summary}")) {
            return exit;
        }
    }
    let bazel_code = match run_bazel(invocation, out, err, workspace, runner, &argv, &[]) {
        Ok(code) => code,
        Err(exit) => return exit,
    };
    if bazel_code != 0 {
        if json {
            if let Err(exit) = emit_built(out, bazel_exit_event(
                CODE_BAZEL_FAILED,
                &format!(
                    "Bazel docs {} failed with exit {bazel_code} (see stderr diagnostics; unit tools/docs/site:user_site, fixture tools/docs/site:demo_site, pinned mdBook 0.4.52)",
                    if invocation.check { "check" } else { "build" },
                ),
                bazel_code,
                "execute",
            )) {
 return kept_child(err, bazel_code, exit);
 }
            if let Err(exit) = emit_event(
                out,
                &command_finished(bazel_code, &FinishedCounts::default()),
            ) {
                return kept_child(err, bazel_code, exit);
            }
        }
        return bazel_code;
    }
    if !invocation.serve {
        if json {
            if let Err(exit) = emit_event(out, &command_finished(0, &FinishedCounts::default())) {
                return exit;
            }
        }
        return 0;
    }
    let port = invocation.port.unwrap_or(DOCS_DEFAULT_PORT);
    let host = invocation.host.as_deref().unwrap_or(DOCS_DEFAULT_HOST);
    let url = preview_url(host, port);
    let serve_dir = match resolve_site_dir(
        &labels,
        &scope_text,
        workspace,
        query_runner,
        &invocation.bazel_startup_options,
    ) {
        Ok(dir) => dir,
        Err(detail) => {
            return operational(invocation, out, err, CODE_SERVE_FAILED, &detail);
        }
    };
    let serve_dir_text = serve_dir.display().to_string();
    if !json && invocation.chatty() {
        if let Err(exit) =
            check_stdout_write(writeln!(out, "Serving docs at {url} ({serve_dir_text})"))
        {
            return exit;
        }
        if invocation.open {
            if let Err(exit) = check_stdout_write(writeln!(out, "Opening {url}")) {
                return exit;
            }
        }
    }
    if invocation.open {
        let open_argv = opener_argv(&url);
        match runner.run(&open_argv, workspace, &[]) {
            Ok(status) if status.code == Some(0) => {}
            Ok(status) => {
                let _ = writeln!(
                    err,
                    "dx: warning: browser open exited with {:?} for {url} (continuing preview)",
                    status.code,
                );
            }
            Err(error) => {
                let _ = writeln!(
                    err,
                    "dx: warning: failed to open browser for {url}: {error} (continuing preview)",
                );
            }
        }
    }
    let serve_argv = vec![
        "python3".to_owned(),
        "-m".to_owned(),
        "http.server".to_owned(),
        port.to_string(),
        "--directory".to_owned(),
        serve_dir_text,
        "--bind".to_owned(),
        host.to_owned(),
    ];
    let serve_status = match runner.run(&serve_argv, workspace, &[]) {
        Ok(status) => status,
        Err(error) => {
            return operational(
                invocation,
                out,
                err,
                CODE_SERVE_FAILED,
                &format!("failed to launch docs preview at {url}: {error} (port in use? retry another --port)"),
            );
        }
    };
    let Some(serve_code) = serve_status.code else {
        return operational(
            invocation,
            out,
            err,
            CODE_BAZEL_SIGNALLED,
            "Docs preview terminated by signal",
        );
    };
    if json {
        if serve_code != 0 {
            if let Err(exit) = emit_built(out, error_event(
                CODE_SERVE_FAILED,
                &format!("Docs preview exited with {serve_code} at {url} (port in use? retry another --port; see stderr diagnostics)"),
                None,
                None,
                Some("serve"),
            )) {
 return kept_child(err, serve_code, exit);
 }
        }
        if let Err(exit) = emit_event(
            out,
            &command_finished(serve_code, &FinishedCounts::default()),
        ) {
            return kept_child(err, serve_code, exit);
        }
    }
    serve_code
}

#[cfg(test)]
mod tests {
    use super::super::test_support::*;
    use super::{CODE_SERVE_FAILED, DOCS_BUILD_TARGET, DOCS_CHECK_TARGET};
    use dx_cli_core::resolve::QueryResult;
    use dx_testing::cli::strings;

    fn script_site_tree(harness: &Harness, rel: &str) {
        harness.write_source(&format!("{rel}/index.html"), "<!doctype html>\n");
        harness.query.outputs.borrow_mut().push(QueryResult {
            code: Some(0),
            stdout: format!("{rel}\n").into_bytes(),
            stderr: Vec::new(),
        });
    }

    fn script_site_query(harness: &Harness, code: Option<i32>, stdout: &str, stderr: &str) {
        harness.query.outputs.borrow_mut().push(QueryResult {
            code,
            stdout: stdout.as_bytes().to_vec(),
            stderr: stderr.as_bytes().to_vec(),
        });
    }

    #[test]
    fn docs_explicit_scopes_and_process_failures_keep_exit_contract() {
        for json in [false, true] {
            let output = if json {
                "--output=json"
            } else {
                "--output=text"
            };
            let harness = Harness::new("docs-scope");
            let (code, out, err) = harness.run(&["docs", "//docs:page", "--check", output]);
            assert_eq!(code, 0, "{out}{err}");
            assert!(out.contains("//docs:page"), "{out}");
            for signalled in [false, true] {
                let mut harness = Harness::new("docs-process-failure");
                harness.signalled = signalled;
                harness.io_error = !signalled;
                let (code, out, err) = harness.run(&["docs", output]);
                assert_eq!(code, 1, "{out}{err}");
                assert!(err.contains(if signalled { "signal" } else { "launch" }));
            }
        }
        let harness = Harness::new("docs-missing-scope");
        let (code, _, err) = harness.run(&["docs", "missing.md"]);
        assert_eq!(code, 2);
        assert!(!err.is_empty());
        assert!(harness.seen_env.borrow().is_empty());
    }

    #[test]
    fn docs_check_builds_aggregate_without_render() {
        let harness = Harness::new("docs-check");
        let (code, out, _) = harness.run(&["docs", "--check", "--output=text"]);
        assert_eq!(code, 0, "{out}");
        assert!(out.contains("Running docs check"), "{out}");
        assert!(out.contains("no render"), "{out}");
        assert!(!out.contains("mdBook render"), "{out}");
    }

    #[test]
    fn docs_build_validates_and_renders() {
        let harness = Harness::new("docs-build");
        let (code, out, _) = harness.run(&["docs", "--output=text"]);
        assert_eq!(code, 0, "{out}");
        assert!(out.contains("Running docs build"), "{out}");
        assert!(out.contains("mdBook render"), "{out}");
    }

    #[test]
    fn docs_default_summary_names_the_target_it_builds() {
        let cases: Vec<(Vec<&str>, &str)> = vec![
            (vec!["docs", "--output=text"], DOCS_BUILD_TARGET),
            (vec!["docs", "--check", "--output=text"], DOCS_CHECK_TARGET),
        ];
        for (words, target) in cases {
            let harness = Harness::new("docs-summary-scope");
            let (code, out, err) = harness.run(&words);
            assert_eq!(code, 0, "{out}{err}");
            assert!(
                out.contains(target),
                "summary must name the label it builds: {out}"
            );
            assert!(
                !out.contains("//..."),
                "must not claim a repository-wide scope: {out}"
            );
        }
    }

    #[test]
    fn docs_forwards_command_options_to_inner_bazel() {
        let harness = Harness::new("docs-forward");
        let inv = invocation(&["docs", "--", "--config=ci", "--jobs=4"]);
        let run = harness.probe_with(&inv, &[Some(0)]);
        assert_eq!(run.code, 0, "{run:?}");
        assert_eq!(run.argv.len(), 1, "{run:?}");
        assert!(run.argv[0].contains(&"--config=ci".to_owned()), "{run:?}");
        assert!(run.argv[0].contains(&"--jobs=4".to_owned()), "{run:?}");
    }

    #[test]
    fn docs_conflicting_and_startup_options_are_pre_exec() {
        for words in [
            vec!["docs", "--", "--home_rc"],
            vec!["docs", "--", "--test_arg=foo"],
        ] {
            let harness = Harness::new("docs-reject");
            let (code, _, err) = harness.run(&words);
            assert_eq!(code, 2, "{words:?}: {err}");
        }
    }

    #[test]
    fn docs_port_requires_serve() {
        let err = dx_cli_core::args::parse(&strings(&["docs", "--port=8080"]))
            .expect_err("port without serve must fail");
        assert!(err.to_string().contains("--port"), "{err}");
        for words in [
            vec!["docs", "--host=example.test"],
            vec!["docs", "--open"],
            vec!["docs", "--port=0", "--serve"],
        ] {
            assert!(
                dx_cli_core::args::parse(&strings(&words)).is_err(),
                "{words:?}"
            );
        }
    }

    #[test]
    fn docs_serve_previews_after_build() {
        let harness = Harness::new("docs-serve");
        script_site_tree(&harness, "bazel-bin/tools/docs/site/user_site");
        let (code, out, _) = harness.run(&["docs", "--serve", "--port=8080"]);
        assert_eq!(code, 0, "{out}");
        assert!(
            out.contains("Serving docs at http://127.0.0.1:8080/"),
            "{out}"
        );
    }

    #[test]
    fn docs_rejects_apply() {
        let err = dx_cli_core::args::parse(&strings(&["docs", "--serve", "--apply"]))
            .expect_err("docs takes no --apply");
        assert!(err.to_string().contains("--apply"), "{err}");
    }

    #[test]
    fn docs_serve_host_and_open() {
        let harness = Harness::new("docs-serve-host");
        script_site_tree(&harness, "bazel-bin/tools/docs/site/user_site");
        let (code, out, _) = harness.run(&[
            "docs",
            "--serve",
            "--port=8080",
            "--host=example.test",
            "--open",
        ]);
        assert_eq!(code, 0, "{out}");
        assert!(
            out.contains("Serving docs at http://example.test:8080/"),
            "{out}"
        );
        assert!(out.contains("Opening http://example.test:8080/"), "{out}");
    }

    #[test]
    fn docs_serve_binds_host_and_opens_browser() {
        use std::cell::RefCell;
        use std::io;
        use std::rc::Rc;
        struct Probe {
            seen: Rc<RefCell<Vec<Vec<String>>>>,
        }
        impl dx_process::Runner for Probe {
            fn run(
                &self,
                argv: &[String],
                _cwd: &std::path::Path,
                _env: &[(&str, &str)],
            ) -> io::Result<dx_process::ChildStatus> {
                self.seen.borrow_mut().push(argv.to_vec());
                Ok(dx_process::ChildStatus::exited(0))
            }
        }
        let harness = Harness::new("docs-serve-bind");
        script_site_tree(&harness, "bazel-bin/tools/docs/site/user_site");
        let inv = invocation(&[
            "docs",
            "--serve",
            "--port=8080",
            "--host=example.test",
            "--open",
        ]);
        let seen = Rc::new(RefCell::new(Vec::new()));
        let runner = Probe {
            seen: Rc::clone(&seen),
        };
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = super::execute_docs(&inv, harness.env(&runner, &mut out, &mut err));
        assert_eq!(code, 0);
        let queries = harness.query.calls.borrow();
        assert_eq!(queries.len(), 1, "{queries:?}");
        assert!(queries[0].contains(&"cquery".to_owned()), "{queries:?}");
        assert!(
            queries[0].contains(&"--output=files".to_owned()),
            "{queries:?}"
        );
        assert!(
            queries[0].contains(&DOCS_BUILD_TARGET.to_owned()),
            "{queries:?}"
        );
        let calls = seen.borrow();
        assert_eq!(calls.len(), 3, "{calls:?}");
        let build = calls
            .iter()
            .find(|argv| argv.contains(&"build".to_owned()))
            .expect("docs build argv");
        for setting in dx_cli_core::plan::BAZEL_SETTINGS {
            assert!(build.contains(&setting.to_owned()), "{build:?}");
        }
        let serve = calls
            .iter()
            .find(|argv| argv.contains(&"http.server".to_owned()));
        let serve = serve.expect("serve argv");
        assert!(serve.contains(&"8080".to_owned()), "{serve:?}");
        assert!(serve.contains(&"--bind".to_owned()), "{serve:?}");
        assert!(serve.contains(&"example.test".to_owned()), "{serve:?}");
        assert!(
            serve
                .iter()
                .any(|arg| arg.ends_with("bazel-bin/tools/docs/site/user_site")),
            "the preview must serve the tree cquery reported: {serve:?}"
        );
        let open = calls
            .iter()
            .find(|argv| argv.iter().any(|arg| arg.contains("webbrowser")));
        assert!(open.is_some(), "opener must run for --open: {calls:?}");
    }

    #[test]
    fn docs_serve_launch_failure_maps_to_serve_failed() {
        use std::io;
        struct LaunchFailRunner;
        impl dx_process::Runner for LaunchFailRunner {
            fn run(
                &self,
                argv: &[String],
                _cwd: &std::path::Path,
                _env: &[(&str, &str)],
            ) -> io::Result<dx_process::ChildStatus> {
                if argv.first().is_some_and(|first| first == "bazel") {
                    Ok(dx_process::ChildStatus::exited(0))
                } else {
                    Err(io::Error::other("fake bind failure: address in use"))
                }
            }
        }
        let harness = Harness::new("docs-serve-bind-fail");
        script_site_tree(&harness, "bazel-bin/tools/docs/site/user_site");
        let inv = invocation(&["docs", "--serve", "--port=8080", "--output=json"]);
        let runner = LaunchFailRunner;
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = super::execute_docs(&inv, harness.env(&runner, &mut out, &mut err));
        assert_eq!(code, 1, "launch failure is operational");
        let err_text = String::from_utf8(err).expect("stderr");
        assert!(err_text.contains("serve_failed"), "{err_text}");
        assert!(!err_text.contains("launch_failed"), "{err_text}");
        let text = String::from_utf8(out).expect("out");
        assert!(text.contains("serve_failed"), "{text}");
    }

    #[test]
    fn docs_json_streams_started_operation_finished() {
        let harness = Harness::new("docs-json");
        let (code, out, _) = harness.run(&["docs", "--check", "--output=json"]);
        assert_eq!(code, 0, "{out}");
        let events = json_events(&out);
        let kinds = event_kinds(&events);
        assert_eq!(kinds[0], "command_started", "{out}");
        assert!(kinds.contains(&"operation"), "{kinds:?}");
        assert_eq!(kinds[kinds.len() - 1], "command_finished", "{out}");
    }

    #[test]
    fn docs_json_pre_exec_errors_have_no_command_started() {
        for argv in [
            &["docs", "--output=json", "--", "--home_rc"][..],
            &["docs", "missing.md", "--output=json"][..],
        ] {
            let harness = Harness::new("docs-pre-exec");
            let (code, out, err) = harness.run(argv);
            assert_eq!(code, 2, "{argv:?}: {out}{err}");
            let events = json_events(&out);
            let kinds = event_kinds(&events);
            assert!(!kinds.contains(&"command_started"), "{argv:?}: {kinds:?}");
            assert!(!kinds.contains(&"operation"), "{argv:?}: {kinds:?}");
            assert_eq!(kinds.first(), Some(&"error"), "{argv:?}: {out}");
            assert_eq!(
                events[0]["code"],
                serde_json::json!("invalid_arguments"),
                "{argv:?}: {out}"
            );
            assert_eq!(kinds.last(), Some(&"command_finished"), "{argv:?}: {out}");
            assert_eq!(
                events[events.len() - 1]["exit_code"],
                serde_json::json!(2),
                "{argv:?}: {out}"
            );
        }
    }

    #[test]
    fn docs_json_launch_failure_is_launch_failed() {
        let mut harness = Harness::new("docs-launch-failed");
        harness.io_error = true;
        let (code, out, err) = harness.run(&["docs", "--output=json"]);
        assert_eq!(code, 1, "{out}{err}");
        let events = json_events(&out);
        let kinds = event_kinds(&events);
        assert_eq!(kinds[0], "command_started", "{out}");
        let operation = kinds.iter().position(|kind| *kind == "operation");
        let error = kinds.iter().position(|kind| *kind == "error");
        assert!(operation.is_some() && operation < error, "{kinds:?}");
        let errors = events_of_kind(&events, "error");
        assert_eq!(errors.len(), 1, "{out}");
        assert_eq!(
            errors[0]["code"],
            serde_json::json!("launch_failed"),
            "{out}"
        );
        assert_eq!(
            errors[0]["cause"],
            serde_json::json!("tool_unavailable"),
            "{out}"
        );
        assert_eq!(kinds[kinds.len() - 1], "command_finished", "{out}");
        assert_eq!(
            events[events.len() - 1]["exit_code"],
            serde_json::json!(1),
            "{out}"
        );
    }

    #[test]
    fn docs_full_disk_is_disk_full_not_bazel_failed() {
        let harness = Harness {
            bazel_code: 36,
            free_disk_bytes: Some(1 << 20),
            ..Harness::new("docs-disk-full")
        };
        let (code, out, err) = harness.run(&["docs", "--output=json"]);
        assert_eq!(code, 36, "{out}{err}");
        let events = json_events(&out);
        let errors = events_of_kind(&events, "error");
        assert_eq!(errors.len(), 1, "{out}");
        assert_eq!(errors[0]["code"], serde_json::json!("disk_full"), "{out}");
        assert_eq!(errors[0]["cause"], serde_json::json!("host"), "{out}");
        assert_eq!(errors[0]["status"], serde_json::json!(36), "{out}");
        assert!(!out.contains("bazel_failed"), "{out}");
        let finished = event(&events, "command_finished");
        assert_eq!(finished["exit_code"], serde_json::json!(36), "{out}");
    }

    #[test]
    fn docs_bazel_failure_names_unit_and_pin() {
        let mut harness = Harness::new("docs-fail");
        harness.bazel_code = 3;
        let (code, out, _) = harness.run(&["docs", "--output=json"]);
        assert_eq!(code, 3, "{out}");
        let events = json_events(&out);
        let error = event(&events, "error");
        assert_eq!(error["code"], serde_json::json!("bazel_failed"), "{out}");
        assert_eq!(error["cause"], serde_json::json!("bazel"), "{out}");
        assert_eq!(error["status"], serde_json::json!(3), "{out}");
        assert_eq!(error["phase"], serde_json::json!("execute"), "{out}");
        assert!(out.contains("tools/docs/site:user_site"), "{out}");
        assert!(out.contains("0.4.52"), "{out}");
    }

    #[test]
    fn docs_serve_previews_the_declared_site_tree() {
        use std::cell::RefCell;
        use std::io;
        use std::rc::Rc;
        struct Probe {
            seen: Rc<RefCell<Vec<Vec<String>>>>,
        }
        impl dx_process::Runner for Probe {
            fn run(
                &self,
                argv: &[String],
                _cwd: &std::path::Path,
                _env: &[(&str, &str)],
            ) -> io::Result<dx_process::ChildStatus> {
                self.seen.borrow_mut().push(argv.to_vec());
                Ok(dx_process::ChildStatus::exited(0))
            }
        }
        fn serve_directory(calls: &[Vec<String>]) -> String {
            let serve = calls
                .iter()
                .find(|argv| argv.contains(&"http.server".to_owned()))
                .expect("serve argv");
            let position = serve
                .iter()
                .position(|arg| arg == "--directory")
                .expect("serve names its directory");
            serve[position + 1].clone()
        }
        let harness = Harness::new("docs-serve-tree");
        script_site_tree(&harness, "bazel-bin/tools/docs/site/user_site");
        let inv = invocation(&["docs", "--serve", "--output=text"]);
        let seen = Rc::new(RefCell::new(Vec::new()));
        let runner = Probe {
            seen: Rc::clone(&seen),
        };
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = super::execute_docs(&inv, harness.env(&runner, &mut out, &mut err));
        assert_eq!(code, 0);
        let directory = serve_directory(&seen.borrow());
        assert!(
            directory.ends_with("bazel-bin/tools/docs/site/user_site"),
            "the preview must serve the tree cquery reported: {directory}"
        );
        let harness = Harness::new("docs-serve-foreign-tree");
        script_site_tree(&harness, "bazel-bin/other/site");
        let inv = invocation(&["docs", "//other:site", "--serve", "--output=text"]);
        let seen = Rc::new(RefCell::new(Vec::new()));
        let runner = Probe {
            seen: Rc::clone(&seen),
        };
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = super::execute_docs(&inv, harness.env(&runner, &mut out, &mut err));
        assert_eq!(code, 0);
        let queries = harness.query.calls.borrow();
        assert_eq!(queries.len(), 1, "{queries:?}");
        assert!(
            queries[0].contains(&"//other:site".to_owned()),
            "{queries:?}"
        );
        let directory = serve_directory(&seen.borrow());
        assert!(
            directory.ends_with("bazel-bin/other/site"),
            "an explicit scope must preview its own tree: {directory}"
        );
        assert!(
            !directory.ends_with("bazel-bin/tools/docs/site/user_site"),
            "an explicit scope must not preview the repository default: {directory}"
        );
    }

    #[test]
    fn docs_serve_cquery_failure_is_serve_failed() {
        for output in ["--output=text", "--output=json"] {
            let harness = Harness::new("docs-serve-cquery-fail");
            script_site_query(
                &harness,
                Some(7),
                "",
                "ERROR: no such target '//tools/docs/site:missing': target not declared\n",
            );
            let (code, out, err) = harness.run(&["docs", "--serve", output]);
            assert_eq!(code, 1, "{out}{err}");
            assert!(err.contains("serve_failed"), "{err}");
            assert!(err.contains("//tools/docs/site:user_site"), "{err}");
            if output == "--output=json" {
                assert!(out.contains("serve_failed"), "{out}");
            }
        }
    }

    #[test]
    fn docs_serve_needs_exactly_one_site_tree() {
        let harness = Harness::new("docs-serve-no-tree");
        script_site_query(&harness, Some(0), "\n", "");
        let (code, _, err) = harness.run(&["docs", "--serve", "--output=text"]);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("serve_failed"), "{err}");
        assert!(err.contains("exactly one"), "{err}");
        let harness = Harness::new("docs-serve-many-trees");
        script_site_query(
            &harness,
            Some(0),
            "bazel-bin/a/site\nbazel-bin/b/site\n",
            "",
        );
        let (code, _, err) = harness.run(&["docs", "--serve", "--output=text"]);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("serve_failed"), "{err}");
        assert!(err.contains("exactly one"), "{err}");
        assert!(err.contains("single site target"), "{err}");
    }

    #[test]
    fn docs_serve_needs_a_rendered_entry_page() {
        let harness = Harness::new("docs-serve-no-entry");
        script_site_query(
            &harness,
            Some(0),
            "bazel-bin/tools/docs/site/user_site\n",
            "",
        );
        let (code, _, err) = harness.run(&["docs", "--serve", "--output=text"]);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("serve_failed"), "{err}");
        assert!(err.contains("index.html"), "{err}");
    }

    #[test]
    fn serve_failed_code_is_stable_single_source() {
        assert_eq!(CODE_SERVE_FAILED, "serve_failed");
    }

    #[test]
    fn docs_serve_failure_emits_serve_failed() {
        use std::io;
        struct ServeFailRunner;
        impl dx_process::Runner for ServeFailRunner {
            fn run(
                &self,
                argv: &[String],
                _cwd: &std::path::Path,
                _env: &[(&str, &str)],
            ) -> io::Result<dx_process::ChildStatus> {
                if argv.first().is_some_and(|first| first == "bazel") {
                    Ok(dx_process::ChildStatus::exited(0))
                } else {
                    Ok(dx_process::ChildStatus::exited(3))
                }
            }
        }
        let harness = Harness::new("docs-serve-fail");
        script_site_tree(&harness, "bazel-bin/tools/docs/site/user_site");
        let inv = invocation(&["docs", "--serve", "--port=8080", "--output=json"]);
        let runner = ServeFailRunner;
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = super::execute_docs(&inv, harness.env(&runner, &mut out, &mut err));
        assert_eq!(code, 3, "{code}");
        let text = String::from_utf8(out).expect("out");
        let events = json_events(&text);
        let kinds = event_kinds(&events);
        assert_eq!(kinds[0], "command_started");
        assert!(kinds.contains(&"error"), "{kinds:?}");
        assert_eq!(kinds[kinds.len() - 1], "command_finished");
        let error = event(&events, "error");
        assert_eq!(error["code"], serde_json::json!(CODE_SERVE_FAILED));
    }
}
