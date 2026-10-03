use super::common::*;
use crate::args::Invocation;
use crate::resolve::resolve;
use dx_output::{
    command_finished, command_started, error_event, operation_event, write_event, FinishedCounts,
    OutputMode,
};
use dx_process::{build_workflow_argv, ProtectedFlag};

const DOCS_CHECK_TARGET: &str = "//docs/site:user_site_aggregate";
const DOCS_BUILD_TARGET: &str = "//docs/site:user_site";

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
        match resolve(&invocation.targets, workspace, query_runner) {
            Ok(resolved) => {
                let scope_text = if resolved.targets.is_empty() {
                    "//...".to_owned()
                } else {
                    resolved.targets.join(" ")
                };
                (resolved.targets, scope_text)
            }
            Err(error) => return pre_exec(err, &error.to_string()),
        }
    };
    let actions: &[&str] = if invocation.check {
        &["extract", "aggregate"]
    } else {
        &["extract", "aggregate", "render"]
    };
    let summary = if invocation.check {
        format!("Running docs check for {scope_text} (extract+aggregate, no render)")
    } else {
        format!("Running docs build for {scope_text} (extract+aggregate+render)")
    };
    let argv = match build_workflow_argv(
        "build",
        &invocation.bazel_options,
        &[crate::plan::workspace_flag()],
        &[ProtectedFlag {
            name: "@rules_dx//config:workspace".to_owned(),
            required: None,
            allowed: Vec::new(),
        }],
        &labels,
    ) {
        Ok(argv) => argv,
        Err(error) => return pre_exec(err, &format!("{error}")),
    };
    if json {
        if let Ok(event) = command_started(invocation.command.name(), invocation.dry_run, mode) {
            let _ = write_event(out, &event);
        }
        for action in actions {
            if let Ok(event) = operation_event(invocation.command.name(), action, Some(&labels)) {
                let _ = write_event(out, &event);
            }
        }
    }
    if invocation.dry_run {
        if json {
            let _ = write_event(out, &command_finished(0, &FinishedCounts::default()));
        } else if invocation.chatty() {
            let _ = writeln!(out, "{summary}");
            if invocation.serve {
                let port = invocation.port.unwrap_or(DOCS_DEFAULT_PORT);
                let host = invocation.host.as_deref().unwrap_or(DOCS_DEFAULT_HOST);
                let url = preview_url(host, port);
                let _ = writeln!(out, "would serve at {url}");
                if invocation.open {
                    let _ = writeln!(out, "would open {url}");
                }
            }
        }
        return 0;
    }
    if !json && invocation.chatty() {
        let _ = writeln!(out, "{summary}");
    }
    let bazel_code = match run_bazel(invocation, out, err, workspace, runner, &argv, &[]) {
        Ok(code) => code,
        Err(exit) => return exit,
    };
    if bazel_code != 0 {
        if json {
            if let Ok(event) = error_event(
                "bazel_failed",
                &format!(
                    "Bazel docs {} failed with exit {bazel_code} (see stderr diagnostics; unit docs/site:user_site, fixture docs/site:demo, pinned mdBook 0.4.43)",
                    if invocation.check { "check" } else { "build" },
                ),
                None,
                None,
                Some("execute"),
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
    if !invocation.serve {
        if json {
            let _ = write_event(out, &command_finished(0, &FinishedCounts::default()));
        }
        return 0;
    }
    let port = invocation.port.unwrap_or(DOCS_DEFAULT_PORT);
    let host = invocation.host.as_deref().unwrap_or(DOCS_DEFAULT_HOST);
    let url = preview_url(host, port);
    let serve_dir = workspace.join("bazel-bin/docs/site");
    let serve_dir_text = serve_dir.display().to_string();
    if !json && invocation.chatty() {
        let _ = writeln!(out, "Serving docs at {url} ({serve_dir_text})");
        if invocation.open {
            let _ = writeln!(out, "Opening {url}");
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
            if let Ok(event) = error_event(
                CODE_SERVE_FAILED,
                &format!("Docs preview exited with {serve_code} at {url} (port in use? retry another --port; see stderr diagnostics)"),
                None,
                None,
                Some("serve"),
            ) {
                let _ = write_event(out, &event);
            }
        }
        let _ = write_event(
            out,
            &command_finished(serve_code, &FinishedCounts::default()),
        );
    }
    serve_code
}

#[cfg(test)]
mod tests {
    use super::super::test_support::*;
    use super::{CODE_SERVE_FAILED, DOCS_BUILD_TARGET, DOCS_CHECK_TARGET};

    #[test]
    fn docs_explicit_scopes_and_process_failures_keep_exit_contract() {
        for json in [false, true] {
            let output = if json {
                "--output=json"
            } else {
                "--output=text"
            };
            let harness = Harness::new("docs-scope");
            let (code, out, err) = harness.run(&[
                "docs",
                "//docs:page",
                "--dry-run",
                "--serve",
                "--open",
                output,
            ]);
            assert_eq!(code, 0, "{out}{err}");
            assert!(out.contains("//docs:page"));
            assert!(harness.seen_env.borrow().is_empty());
            if !json {
                assert!(out.contains("would open http://127.0.0.1:8000/"));
            }
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
    }

    #[test]
    fn docs_build_validates_and_renders() {
        let harness = Harness::new("docs-build");
        let (code, out, _) = harness.run(&["docs", "--output=text"]);
        assert_eq!(code, 0, "{out}");
        assert!(out.contains("Running docs build"), "{out}");
        assert!(out.contains("render"), "{out}");
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
        use std::cell::RefCell;
        use std::rc::Rc;
        let harness = Harness::new("docs-forward");
        let seen = Rc::new(RefCell::new(Vec::new()));
        let probe = ArgvProbe {
            code: Some(0),
            seen: Rc::clone(&seen),
        };
        let inv = invocation(&["docs", "--", "--config=ci", "--jobs=4"]);
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = super::execute_docs(
            &inv,
            super::super::common::Env {
                workspace: &harness.workspace,
                runner: &probe,
                query_runner: &harness.query,
                temp_dir: &harness.temp,
                pid: std::process::id(),
                nonce: 0,
                out: &mut out,
                err: &mut err,
                ci: false,
            },
        );
        assert_eq!(code, 0);
        let seen = seen.borrow();
        assert_eq!(seen.len(), 1, "{seen:?}");
        assert!(seen[0].contains(&"--config=ci".to_owned()), "{:?}", seen[0]);
        assert!(seen[0].contains(&"--jobs=4".to_owned()), "{:?}", seen[0]);
    }

    #[test]
    fn docs_conflicting_and_startup_options_are_pre_exec() {
        for words in [
            vec!["docs", "--", "--@rules_dx//config:workspace=//other:config"],
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
        let args: Vec<String> = ["docs", "--port=8080"]
            .iter()
            .map(ToString::to_string)
            .collect();
        let err = crate::args::parse(&args).expect_err("port without serve must fail");
        assert!(err.to_string().contains("--port"), "{err}");
        for words in [
            vec!["docs", "--host=example.test"],
            vec!["docs", "--open"],
            vec!["docs", "--port=0", "--serve"],
        ] {
            let args: Vec<String> = words.iter().map(ToString::to_string).collect();
            assert!(crate::args::parse(&args).is_err(), "{words:?}");
        }
    }

    #[test]
    fn docs_serve_previews_after_build() {
        let harness = Harness::new("docs-serve");
        let (code, out, _) = harness.run(&["docs", "--serve", "--port=8080"]);
        assert_eq!(code, 0, "{out}");
        assert!(
            out.contains("Serving docs at http://127.0.0.1:8080/"),
            "{out}"
        );
    }

    #[test]
    fn docs_serve_host_and_open() {
        let harness = Harness::new("docs-serve-host");
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
                Ok(dx_process::ChildStatus { code: Some(0) })
            }
        }
        let harness = Harness::new("docs-serve-bind");
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
        let code = super::execute_docs(
            &inv,
            super::super::common::Env {
                workspace: &harness.workspace,
                runner: &runner,
                query_runner: &harness.query,
                temp_dir: &harness.temp,
                pid: std::process::id(),
                nonce: 0,
                out: &mut out,
                err: &mut err,
                ci: false,
            },
        );
        assert_eq!(code, 0);
        let calls = seen.borrow();
        assert_eq!(calls.len(), 3, "{calls:?}");
        let serve = calls
            .iter()
            .find(|argv| argv.contains(&"http.server".to_owned()));
        let serve = serve.expect("serve argv");
        assert!(serve.contains(&"8080".to_owned()), "{serve:?}");
        assert!(serve.contains(&"--bind".to_owned()), "{serve:?}");
        assert!(serve.contains(&"example.test".to_owned()), "{serve:?}");
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
                    Ok(dx_process::ChildStatus { code: Some(0) })
                } else {
                    Err(io::Error::other("fake bind failure: address in use"))
                }
            }
        }
        let harness = Harness::new("docs-serve-bind-fail");
        let inv = invocation(&["docs", "--serve", "--port=8080", "--output=json"]);
        let runner = LaunchFailRunner;
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = super::execute_docs(
            &inv,
            super::super::common::Env {
                workspace: &harness.workspace,
                runner: &runner,
                query_runner: &harness.query,
                temp_dir: &harness.temp,
                pid: std::process::id(),
                nonce: 0,
                out: &mut out,
                err: &mut err,
                ci: false,
            },
        );
        assert_eq!(code, 1, "launch failure is operational");
        let err_text = String::from_utf8(err).expect("stderr");
        assert!(err_text.contains("serve_failed"), "{err_text}");
        assert!(!err_text.contains("launch_failed"), "{err_text}");
        let text = String::from_utf8(out).expect("out");
        assert!(text.contains("serve_failed"), "{text}");
    }

    #[test]
    fn docs_dry_run_plans_without_launch() {
        let harness = Harness::new("docs-dry");
        let (code, out, _) = harness.run(&["docs", "--check", "--dry-run"]);
        assert_eq!(code, 0, "{out}");
        assert!(out.contains("Running docs check"), "{out}");
        assert!(
            harness.seen_env.borrow().is_empty(),
            "dry-run launches nothing"
        );
    }

    #[test]
    fn docs_json_streams_started_operation_finished() {
        let harness = Harness::new("docs-json");
        let (code, out, _) = harness.run(&["docs", "--check", "--output=json"]);
        assert_eq!(code, 0, "{out}");
        assert!(out.contains("command_started"), "{out}");
        assert!(out.contains("operation"), "{out}");
        assert!(out.contains("command_finished"), "{out}");
        for line in out.lines() {
            serde_json::from_str::<serde_json::Value>(line).expect("NDJSON line");
        }
    }

    #[test]
    fn docs_bazel_failure_names_unit_and_pin() {
        let mut harness = Harness::new("docs-fail");
        harness.bazel_code = 3;
        let (code, out, _) = harness.run(&["docs", "--output=json"]);
        assert_eq!(code, 3, "{out}");
        assert!(out.contains("bazel_failed"), "{out}");
        assert!(out.contains("docs/site:user_site"), "{out}");
        assert!(out.contains("0.4.43"), "{out}");
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
                    Ok(dx_process::ChildStatus { code: Some(0) })
                } else {
                    Ok(dx_process::ChildStatus { code: Some(3) })
                }
            }
        }
        let harness = Harness::new("docs-serve-fail");
        let inv = invocation(&["docs", "--serve", "--port=8080", "--output=json"]);
        let runner = ServeFailRunner;
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = super::execute_docs(
            &inv,
            super::super::common::Env {
                workspace: &harness.workspace,
                runner: &runner,
                query_runner: &harness.query,
                temp_dir: &harness.temp,
                pid: std::process::id(),
                nonce: 0,
                out: &mut out,
                err: &mut err,
                ci: false,
            },
        );
        assert_eq!(code, 3, "{code}");
        let text = String::from_utf8(out).expect("out");
        let events: Vec<serde_json::Value> = text
            .lines()
            .map(serde_json::from_str)
            .collect::<Result<_, _>>()
            .expect("NDJSON");
        let kinds: Vec<&str> = events
            .iter()
            .map(|event| event["event"].as_str().expect("event"))
            .collect();
        assert_eq!(kinds[0], "command_started");
        assert!(kinds.contains(&"error"), "{kinds:?}");
        assert_eq!(kinds[kinds.len() - 1], "command_finished");
        let error = events
            .iter()
            .find(|event| event["event"] == serde_json::json!("error"))
            .expect("serve error");
        assert_eq!(error["code"], serde_json::json!(CODE_SERVE_FAILED));
    }
}
