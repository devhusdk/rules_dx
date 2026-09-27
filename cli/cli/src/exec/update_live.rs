use super::super::test_support::*;
use dx_process::{ChildStatus, Runner};
use std::cell::RefCell;
use std::collections::HashMap;
use std::io;
use std::path::Path;
use std::rc::Rc;

pub(super) struct ScriptRunner {
    pub(super) calls: Rc<RefCell<Vec<Vec<String>>>>,
    pub(super) codes: RefCell<HashMap<String, Option<i32>>>,
    pub(super) io_error: bool,
}

impl ScriptRunner {
    pub(super) fn new(codes: &[(&str, Option<i32>)]) -> Self {
        ScriptRunner {
            calls: Rc::new(RefCell::new(Vec::new())),
            codes: RefCell::new(
                codes
                    .iter()
                    .map(|(key, code)| ((*key).to_owned(), *code))
                    .collect(),
            ),
            io_error: false,
        }
    }

    pub(super) fn key_for(argv: &[String]) -> String {
        if argv.contains(&"//rust/tests/fixtures/hello:hello".to_owned()) {
            "cargo".to_owned()
        } else if argv.contains(&"@pnpm//:pnpm".to_owned()) {
            "npm".to_owned()
        } else if argv.contains(&"@maven//:pin".to_owned()) {
            "maven".to_owned()
        } else if argv.iter().any(|arg| arg.contains("paket2bazel")) {
            "nuget".to_owned()
        } else {
            argv.join(" ")
        }
    }
}

impl Runner for ScriptRunner {
    fn run(&self, argv: &[String], _cwd: &Path, _env: &[(&str, &str)]) -> io::Result<ChildStatus> {
        if self.io_error {
            return Err(io::Error::other("fake launch failure"));
        }
        self.calls.borrow_mut().push(argv.to_vec());
        let key = Self::key_for(argv);
        let code = self.codes.borrow().get(&key).copied().unwrap_or(Some(0));
        Ok(ChildStatus { code })
    }
}

pub(super) fn run_with(argv: &[&str], runner: &ScriptRunner) -> (i32, String, String) {
    use crate::args::parse;
    use crate::exec::{execute, Env};
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let id = COUNTER.fetch_add(1, Ordering::SeqCst);
    let words: Vec<String> = argv.iter().map(|word| (*word).to_owned()).collect();
    let invocation = parse(&words).expect("parse");
    let workspace_guard = temp_dir(&format!("update-live-{id}"));
    let temp_guard = temp_dir(&format!("update-live-tmp-{id}"));
    let query = ScriptQuery {
        calls: RefCell::new(Vec::new()),
        outputs: RefCell::new(Vec::new()),
    };
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = execute(
        &invocation,
        Env {
            workspace: workspace_guard.path(),
            runner,
            query_runner: &query,
            temp_dir: temp_guard.path(),
            pid: std::process::id(),
            nonce: 0,
            out: &mut out,
            err: &mut err,
            ci: false,
        },
    );
    (
        code,
        String::from_utf8(out).expect("stdout"),
        String::from_utf8(err).expect("stderr"),
    )
}

#[test]
pub(super) fn dry_run_resolves_without_launching() {
    let harness = Harness::new("update-dryrun-live");
    let (code, out, err) = harness.run(&["update", "--dry-run"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(
        out.contains("Running update for all dependency sets"),
        "{out}"
    );
    assert_eq!(err, "", "{err}");
    assert!(
        harness.seen_env.borrow().is_empty(),
        "dry-run launches nothing"
    );

    let harness = Harness::new("update-dryrun-npm");
    let (code, out, err) = harness.run(&["update", "npm:jest", "--dry-run"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("Running update for npm:jest"), "{out}");
    assert_eq!(err, "", "{err}");
}

#[test]
pub(super) fn dry_run_rejects_unknown_and_unowned() {
    let harness = Harness::new("update-dryrun-unknown");
    let (code, _, err) = harness.run(&["update", "crates", "--dry-run"]);
    assert_eq!(code, 2, "{err}");
    let harness = Harness::new("update-dryrun-unowned");
    let (code, _, err) = harness.run(&[
        "update",
        "python/tests/fixtures/hello/hello.py",
        "--dry-run",
    ]);
    assert_eq!(code, 2, "{err}");
}

#[test]
pub(super) fn live_all_success_reports_per_set_and_exits_zero() {
    let runner = ScriptRunner::new(&[]);
    let (code, out, err) = run_with(&["update"], &runner);
    assert_eq!(code, 0, "{out}{err}");
    assert!(
        out.contains("Running update for all dependency sets"),
        "{out}"
    );
    assert!(out.contains("updated cargo ("), "{out}");
    assert!(out.contains("updated npm ("), "{out}");
    assert!(out.contains("updated maven ("), "{out}");
    assert!(out.contains("updated nuget ("), "{out}");
    assert!(
        out.contains("updated go (pinned module lock; no-op success)"),
        "{out}"
    );
    assert!(out.contains("5 succeeded, 0 failed, 0 blocked"), "{out}");
    assert_eq!(err, "", "{err}");
    assert_eq!(runner.calls.borrow().len(), 4);
}

#[test]
pub(super) fn live_independent_failure_preserves_success_and_exits_one() {
    let runner = ScriptRunner::new(&[("maven", Some(1))]);
    let (code, out, err) = run_with(&["update"], &runner);
    assert_eq!(code, 1, "{out}{err}");
    assert!(out.contains("updated cargo ("), "{out}");
    assert!(out.contains("updated npm ("), "{out}");
    assert!(out.contains("4 succeeded, 1 failed, 0 blocked"), "{out}");
    assert!(err.contains("update_failed"), "{err}");
    assert!(err.contains("failed to update maven"), "{err}");
    assert!(err.contains("update_recovery"), "{err}");
    assert!(err.contains("dx update maven"), "{err}");
    assert_eq!(runner.calls.borrow().len(), 4);
}

#[test]
pub(super) fn live_selective_npm_runs_once_with_packages() {
    let runner = ScriptRunner::new(&[]);
    let (code, out, err) = run_with(&["update", "npm:jest", "npm:react"], &runner);
    assert_eq!(code, 0, "{out}{err}");
    assert!(
        out.contains("Running update for npm:jest, npm:react"),
        "{out}"
    );
    assert!(
        out.contains("updated npm:jest, npm:react (pnpm-lock.yaml)"),
        "{out}"
    );
    assert_eq!(runner.calls.borrow().len(), 1);
    assert!(runner.calls.borrow()[0].contains(&"jest".to_owned()));
    assert!(runner.calls.borrow()[0].contains(&"react".to_owned()));
}

#[test]
pub(super) fn live_unsupported_selective_fails_without_launch() {
    let runner = ScriptRunner::new(&[]);
    let (code, out, err) = run_with(&["update", "cargo:anyhow"], &runner);
    assert_eq!(code, 1, "{out}{err}");
    assert!(err.contains("update_failed"), "{err}");
    assert!(err.contains("unsupported"), "{err}");
    assert!(runner.calls.borrow().is_empty());
}

#[test]
pub(super) fn live_unsupported_nuget_selective_fails_without_launch() {
    let runner = ScriptRunner::new(&[]);
    let (code, out, err) = run_with(&["update", "nuget:FSharp.Core"], &runner);
    assert_eq!(code, 1, "{out}{err}");
    assert!(err.contains("update_failed"), "{err}");
    assert!(err.contains("unsupported"), "{err}");
    assert!(err.contains("dx update nuget"), "{err}");
    assert!(runner.calls.borrow().is_empty());
}

#[test]
pub(super) fn live_unsupported_go_selective_fails_without_launch() {
    let runner = ScriptRunner::new(&[]);
    let (code, out, err) = run_with(&["update", "go:github.com/google/go-cmp/cmp"], &runner);
    assert_eq!(code, 1, "{out}{err}");
    assert!(err.contains("update_failed"), "{err}");
    assert!(err.contains("unsupported"), "{err}");
    assert!(err.contains("dx bump gomod"), "{err}");
    assert!(runner.calls.borrow().is_empty());
}

#[test]
pub(super) fn live_target_resolves_to_owning_set_only() {
    let runner = ScriptRunner::new(&[]);
    let (code, out, err) = run_with(&["update", "//go/tests/fixtures/hello:hello"], &runner);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("Running update for go"), "{out}");
    assert!(
        out.contains("updated go (pinned module lock; no-op success)"),
        "{out}"
    );
    assert!(runner.calls.borrow().is_empty());
}

#[test]
pub(super) fn live_json_emits_per_set_notices_and_finished() {
    let runner = ScriptRunner::new(&[("npm", Some(2))]);
    let (code, out, err) = run_with(&["update", "--output=json"], &runner);
    assert_eq!(code, 1, "{out}{err}");
    let events: Vec<serde_json::Value> = out
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()
        .expect("NDJSON");
    let kinds: Vec<&str> = events
        .iter()
        .map(|event| event["event"].as_str().expect("event"))
        .collect();
    assert_eq!(kinds[0], "command_started");
    assert_eq!(kinds[kinds.len() - 1], "command_finished");
    assert_eq!(
        events.last().expect("finished")["exit_code"],
        serde_json::json!(1)
    );
    assert!(kinds.contains(&"notice"));
    assert!(kinds.contains(&"error"));
    assert!(err.contains("update_failed"), "{err}");
    assert!(out.contains("\"code\":\"update_recovery\""), "{out}");
    assert!(err.contains("update_recovery"), "{err}");
    for event in &events {
        let code = event
            .get("code")
            .and_then(|code| code.as_str())
            .unwrap_or("");
        if code == "update_set_success"
            || code == "update_set_blocked"
            || event.get("code").is_none() && event["event"] == serde_json::json!("error")
        {
            let correlation = event["correlation"].as_str().expect("correlation");
            assert!(correlation.starts_with("update:"), "{event}");
        }
    }
    assert!(out.contains("\"correlation\":\"update:cargo\""), "{out}");
    assert!(out.contains("\"correlation\":\"update:npm\""), "{out}");
}

#[test]
pub(super) fn manifest_projects_to_correlated_change_and_mutation() {
    const DIGEST: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    let manifest = dx_update::manifest::CommittedManifest {
        set: "npm".to_owned(),
        changes: vec![dx_update::manifest::CommittedChange {
            path: "pnpm-lock.yaml".to_owned(),
            kind: dx_update::manifest::CommittedKind::Modify,
            source_digest: Some(DIGEST.to_owned()),
            old_len: 3,
            new_content: "new\n".to_owned(),
        }],
    };
    let events = super::project_manifest_events(&manifest).expect("project");
    assert_eq!(events.len(), 2);
    assert_eq!(events[0]["event"], serde_json::json!("change"));
    assert_eq!(events[0]["path"], serde_json::json!("pnpm-lock.yaml"));
    assert_eq!(events[0]["correlation"], serde_json::json!("update:npm"));
    assert_eq!(
        events[0]["edits"],
        serde_json::json!([{"start_byte": 0, "end_byte": 3, "replacement": "new\n"}])
    );
    assert_eq!(events[1]["event"], serde_json::json!("mutation"));
    assert_eq!(events[1]["outcome"], serde_json::json!("applied"));
    assert_eq!(events[1]["correlation"], serde_json::json!("update:npm"));
    let empty = dx_update::manifest::CommittedManifest {
        set: "go".to_owned(),
        changes: Vec::new(),
    };
    assert!(super::project_manifest_events(&empty)
        .expect("empty")
        .is_empty());
}

#[test]
pub(super) fn live_dry_run_json_still_plans_without_per_set() {
    let harness = Harness::new("update-dryrun-json-live");
    let (code, out, err) = harness.run(&["update", "--dry-run", "--output=json"]);
    assert_eq!(code, 0, "{out}{err}");
    let events: Vec<serde_json::Value> = out
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()
        .expect("NDJSON");
    let kinds: Vec<&str> = events
        .iter()
        .map(|event| event["event"].as_str().expect("event"))
        .collect();
    assert_eq!(kinds, vec!["command_started", "command_finished"]);
}
