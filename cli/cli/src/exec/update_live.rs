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
        } else if argv.contains(&"@pnpm//:pnpm".to_owned())
            && argv
                .iter()
                .any(|arg| arg.contains("quality/tools/javascript"))
        {
            "npm-tools".to_owned()
        } else if argv.contains(&"@pnpm//:pnpm".to_owned()) {
            "npm".to_owned()
        } else if argv.contains(&"@maven//:pin".to_owned()) {
            "maven".to_owned()
        } else if argv.iter().any(|arg| arg.contains("paket2bazel")) {
            "nuget".to_owned()
        } else if argv.contains(&"uv".to_owned())
            && argv
                .iter()
                .any(|arg| arg.contains("python/tests/fixtures/hello"))
        {
            "uv".to_owned()
        } else if argv.contains(&"uv".to_owned())
            && argv.iter().any(|arg| arg.contains("quality/tools/python"))
        {
            "uv-tools".to_owned()
        } else if argv.contains(&"uv".to_owned())
            && argv.iter().any(|arg| arg.contains("examples/adopt-python"))
        {
            "uv-adopt".to_owned()
        } else if argv.contains(&"uv".to_owned())
            && argv
                .iter()
                .any(|arg| arg.contains("examples/adopt-polyglot"))
        {
            "uv-adopt-polyglot".to_owned()
        } else if argv.iter().any(|arg| arg.contains("examples/adopt-js-ts")) {
            "npm-adopt".to_owned()
        } else if argv
            .iter()
            .any(|arg| arg.contains("examples/adopt-polyglot"))
        {
            "npm-adopt-polyglot".to_owned()
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
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let id = COUNTER.fetch_add(1, Ordering::SeqCst);
    let words: Vec<String> = argv.iter().map(|word| (*word).to_owned()).collect();
    let invocation = parse(&words).expect("parse");
    let harness = Harness::new(&format!("update-live-{id}"));
    harness.execute_with(&invocation, runner)
}

#[test]
pub(super) fn dry_run_resolves_without_launching() {
    let harness = Harness::new("update-dryrun-live");
    let (code, out, err) = harness.run(&["update", "--apply", "--dry-run"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(
        out.contains("Running update for all dependency sets"),
        "{out}"
    );
    assert!(
        out.contains("Would run resolvers for the selected sets"),
        "{out}"
    );
    assert_eq!(err, "", "{err}");
    assert!(
        harness.seen_env.borrow().is_empty(),
        "dry-run launches nothing"
    );

    let harness = Harness::new("update-dryrun-npm");
    let (code, out, err) = harness.run(&["update", "--apply", "npm:jest", "--dry-run"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("Running update for npm:jest"), "{out}");
    assert_eq!(err, "", "{err}");
}

#[test]
pub(super) fn dry_run_rejects_unknown_and_unowned() {
    let harness = Harness::new("update-dryrun-unknown");
    let (code, _, err) = harness.run(&["update", "--apply", "crates", "--dry-run"]);
    assert_eq!(code, 2, "{err}");
    let harness = Harness::new("update-dryrun-unowned");
    let (code, _, err) = harness.run(&["update", "--apply", "docs/cli/README.md", "--dry-run"]);
    assert_eq!(code, 2, "{err}");
}

#[test]
pub(super) fn live_all_success_reports_per_set_and_exits_zero() {
    let runner = ScriptRunner::new(&[]);
    let (code, out, err) = run_with(&["update", "--apply"], &runner);
    assert_eq!(code, 0, "{out}{err}");
    assert!(
        out.contains("Running update for all dependency sets"),
        "{out}"
    );
    assert!(out.contains("updated cargo ("), "{out}");
    assert!(out.contains("updated npm ("), "{out}");
    assert!(out.contains("updated npm-tools ("), "{out}");
    assert!(out.contains("updated maven ("), "{out}");
    assert!(out.contains("updated nuget ("), "{out}");
    assert!(out.contains("updated uv ("), "{out}");
    assert!(out.contains("updated uv-tools ("), "{out}");
    assert!(out.contains("updated npm-adopt ("), "{out}");
    assert!(out.contains("updated uv-adopt ("), "{out}");
    assert!(
        out.contains("go is pinned (third_party/go/go.mod, third_party/go/go.sum); left untouched"),
        "{out}"
    );
    assert!(
        out.contains("ruby is pinned (third_party/ruby/Gemfile.lock"),
        "{out}"
    );
    assert!(
        out.contains("powershell is pinned (third_party/powershell/PSGallery.lock.json)"),
        "{out}"
    );
    assert!(!out.contains("no-op success"), "{out}");
    assert!(out.contains("14 succeeded, 0 failed, 0 blocked"), "{out}");
    assert_eq!(err, "", "{err}");
    assert_eq!(runner.calls.borrow().len(), 11);
}

#[test]
pub(super) fn live_independent_failure_preserves_success_and_exits_one() {
    let runner = ScriptRunner::new(&[("maven", Some(1))]);
    let (code, out, err) = run_with(&["update", "--apply"], &runner);
    assert_eq!(code, 1, "{out}{err}");
    assert!(out.contains("updated cargo ("), "{out}");
    assert!(out.contains("updated npm ("), "{out}");
    assert!(out.contains("13 succeeded, 1 failed, 0 blocked"), "{out}");
    assert!(err.contains("update_failed"), "{err}");
    assert!(err.contains("failed to update maven"), "{err}");
    assert!(err.contains("update_recovery"), "{err}");
    assert!(err.contains("dx update --apply maven"), "{err}");
    assert_eq!(runner.calls.borrow().len(), 11);
}

#[test]
pub(super) fn live_selective_npm_runs_once_with_packages() {
    let runner = ScriptRunner::new(&[]);
    let (code, out, err) = run_with(&["update", "--apply", "npm:jest", "npm:react"], &runner);
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
    let (code, out, err) = run_with(&["update", "--apply", "cargo:anyhow"], &runner);
    assert_eq!(code, 1, "{out}{err}");
    assert!(err.contains("update_unsupported"), "{err}");
    assert!(err.contains("unsupported"), "{err}");
    assert!(runner.calls.borrow().is_empty());
}

#[test]
pub(super) fn live_unsupported_nuget_selective_fails_without_launch() {
    let runner = ScriptRunner::new(&[]);
    let (code, out, err) = run_with(&["update", "--apply", "nuget:FSharp.Core"], &runner);
    assert_eq!(code, 1, "{out}{err}");
    assert!(err.contains("update_unsupported"), "{err}");
    assert!(err.contains("unsupported"), "{err}");
    assert!(err.contains("dx update nuget"), "{err}");
    assert!(runner.calls.borrow().is_empty());
}

#[test]
pub(super) fn live_unsupported_go_selective_fails_without_launch() {
    let runner = ScriptRunner::new(&[]);
    let (code, out, err) = run_with(&["update", "--apply", "go:github.com/google/go-cmp/cmp"], &runner);
    assert_eq!(code, 1, "{out}{err}");
    assert!(err.contains("update_unsupported"), "{err}");
    assert!(err.contains("unsupported"), "{err}");
    assert!(err.contains("dx bump gomod"), "{err}");
    assert!(runner.calls.borrow().is_empty());
}

#[test]
pub(super) fn live_target_resolves_to_owning_set_only() {
    let runner = ScriptRunner::new(&[]);
    let (code, out, err) = run_with(&["update", "--apply", "//go/tests/fixtures/hello:hello"], &runner);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("Running update for go"), "{out}");
    assert!(out.contains("go is pinned"), "{out}");
    assert!(!out.contains("no-op success"), "{out}");
    assert!(runner.calls.borrow().is_empty());
}

#[test]
pub(super) fn live_pinned_sets_never_launch_and_never_claim_resolver_freshness() {
    for set in ["go", "ruby", "powershell"] {
        let runner = ScriptRunner::new(&[]);
        let (code, out, err) = run_with(&["update", "--apply", set], &runner);
        assert_eq!(code, 0, "{set}: {out}{err}");
        assert!(out.contains("is pinned"), "{set}: {out}");
        assert!(out.contains("left untouched"), "{set}: {out}");
        assert!(!out.contains("updated "), "{set}: {out}");
        assert!(!out.contains("no-op success"), "{set}: {out}");
        assert!(runner.calls.borrow().is_empty(), "{set} launches nothing");
    }
}

#[test]
pub(super) fn live_json_emits_per_set_notices_and_finished() {
    let runner = ScriptRunner::new(&[("npm", Some(2))]);
    let (code, out, err) = run_with(&["update", "--apply", "--output=json"], &runner);
    assert_eq!(code, 1, "{out}{err}");
    let events = json_events(&out);
    let kinds = event_kinds(&events);
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
            || code == "update_set_pinned"
            || code == "update_set_current"
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
    let (code, out, err) = harness.run(&["update", "--apply", "--dry-run", "--output=json"]);
    assert_eq!(code, 0, "{out}{err}");
    let events = json_events(&out);
    let kinds = event_kinds(&events);
    assert_eq!(kinds, vec!["command_started", "command_finished"]);
}

#[test]
pub(super) fn updater_spawn_and_signal_failures_keep_other_sets_independent() {
    for spawn_error in [false, true] {
        let mut runner = ScriptRunner::new(&[("cargo", None)]);
        runner.io_error = spawn_error;
        let (code, out, err) = run_with(&["update", "--apply", "cargo", "go", "--output=json"], &runner);
        assert_eq!(code, 1, "{out}{err}");
        assert!(out.contains(if spawn_error {
            "failed to launch updater"
        } else {
            "terminated by signal"
        }));
        let events: Vec<serde_json::Value> = out
            .lines()
            .map(|line| serde_json::from_str(line).expect("event"))
            .collect();
        assert!(
            events
                .iter()
                .any(|event| event["code"] == "update_set_pinned"
                    && event["scope"] == serde_json::json!(["go"])),
            "{out}"
        );
        assert_eq!(events.last().expect("finished")["exit_code"], 1);
    }
}

#[test]
pub(super) fn apply_offline_fails_with_offline_required_without_launching() {
    let runner = ScriptRunner::new(&[]);
    let (code, out, err) = run_with(&["update", "--apply", "cargo", "--offline"], &runner);
    assert_eq!(code, 1, "{out}{err}");
    assert!(err.contains("offline_required"), "{err}");
    assert!(err.contains("cannot update cargo without network"), "{err}");
    assert!(runner.calls.borrow().is_empty(), "offline launches nothing");
    let (code, out, err) = run_with(&["update", "--apply", "cargo", "--offline", "--output=json"], &runner);
    assert_eq!(code, 1, "{out}{err}");
    assert!(out.contains("\"code\":\"offline_required\""), "{out}");
    assert!(!out.contains("\"code\":\"update_failed\""), "{out}");
    let go_runner = ScriptRunner::new(&[]);
    let (code, out, err) = run_with(&["update", "--apply", "go", "--offline"], &go_runner);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("go is pinned"), "{out}");
    assert!(
        go_runner.calls.borrow().is_empty(),
        "go pinned launches nothing"
    );
}

#[test]
pub(super) fn apply_offline_required_code_is_stable_single_source() {
    assert_eq!(
        crate::exec::common::CODE_OFFLINE_REQUIRED,
        "offline_required"
    );
}

#[test]
pub(super) fn apply_json_never_emits_change_or_mutation() {
    let runner = ScriptRunner::new(&[]);
    let (code, out, err) = run_with(&["update", "--apply", "--output=json"], &runner);
    assert_eq!(code, 0, "{out}{err}");
    assert!(
        !out.contains("\"event\":\"change\""),
        "update must not emit change events: {out}"
    );
    assert!(
        !out.contains("\"event\":\"mutation\""),
        "update must not emit mutation events: {out}"
    );
    assert!(
        !out.contains("\"event\":\"diagnostic\""),
        "update must not emit diagnostics: {out}"
    );
    assert!(
        !out.contains("\"event\":\"operation\""),
        "update must not emit operations: {out}"
    );
    let events = json_events(&out);
    for kind in event_kinds(&events) {
        assert!(
            kind == "command_started"
                || kind == "notice"
                || kind == "error"
                || kind == "command_finished",
            "unexpected update event {kind}: {out}"
        );
    }
}

#[test]
pub(super) fn apply_json_completeness_is_per_set_plus_finished() {
    let runner = ScriptRunner::new(&[("maven", Some(1))]);
    let (code, out, err) = run_with(&["update", "--apply", "--output=json"], &runner);
    assert_eq!(code, 1, "{out}{err}");
    let events = json_events(&out);
    let kinds = event_kinds(&events);
    assert_eq!(kinds[0], "command_started");
    assert_eq!(kinds[kinds.len() - 1], "command_finished");
    let middle = &events[1..events.len() - 1];
    assert_eq!(middle.len(), 15, "{out}");
    let (per_set, recovery) = (&middle[..14], &middle[14]);
    assert_eq!(
        recovery["code"],
        serde_json::json!("update_recovery"),
        "{out}"
    );
    assert_eq!(recovery["event"], serde_json::json!("notice"), "{out}");
    let recovery_message = recovery["message"].as_str().expect("message");
    assert!(recovery_message.contains("dx update --apply maven"), "{out}");
    assert!(recovery_message.contains("idempotent"), "{out}");
    assert!(err.contains("update_recovery"), "{err}");
    let scopes: Vec<String> = per_set
        .iter()
        .map(|event| {
            let kind = event["event"].as_str().expect("event");
            if kind == "error" {
                assert_eq!(
                    event["code"].as_str().expect("code"),
                    "update_failed",
                    "{out}"
                );
                let message = event["message"].as_str().expect("message");
                assert!(message.contains("maven"), "{out}");
                "maven".to_owned()
            } else {
                assert_eq!(kind, "notice", "{out}");
                let code = event["code"].as_str().expect("code");
                assert!(
                    code == "update_set_success" || code == "update_set_pinned",
                    "{out}"
                );
                event["scope"][0].as_str().expect("scope").to_owned()
            }
        })
        .collect();
    let mut sorted = scopes.clone();
    sorted.sort();
    assert_eq!(scopes, sorted, "per-set events use sorted set order: {out}");
    let finished = events.last().expect("finished");
    assert_eq!(finished["exit_code"], serde_json::json!(1));
    assert_eq!(finished["results_complete"], serde_json::json!(true));
    assert!(finished.get("changes").is_none(), "{out}");
    assert!(finished.get("mutations").is_none(), "{out}");
    assert!(finished.get("diagnostics").is_none(), "{out}");
}

#[test]
pub(super) fn apply_success_emits_no_recovery() {
    let runner = ScriptRunner::new(&[]);
    let (code, out, err) = run_with(&["update", "--apply"], &runner);
    assert_eq!(code, 0, "{out}{err}");
    assert!(!err.contains("update_recovery"), "{err}");
    assert!(!out.contains("update_recovery"), "{out}");

    let runner = ScriptRunner::new(&[]);
    let (code, out, err) = run_with(&["update", "--apply", "--output=json"], &runner);
    assert_eq!(code, 0, "{out}{err}");
    assert!(!out.contains("update_recovery"), "{out}");
}

#[test]
pub(super) fn apply_offline_dry_run_plans_cache_only_without_launching() {
    let harness = Harness::new("update-offline-dryrun");
    let (code, out, err) = harness.run(&["update", "--apply", "--offline", "--dry-run"]);
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(
        out,
        "Running update for all dependency sets (offline, cache-only)\n\
         Would run resolvers for the selected sets\n",
        "{out}"
    );
    assert_eq!(err, "", "{err}");
    assert!(
        harness.seen_env.borrow().is_empty(),
        "offline dry-run launches nothing"
    );
    let alias = Harness::new("update-frozen-dryrun");
    let (code, out, err) = alias.run(&["update", "--apply", "--frozen", "--dry-run"]);
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(
        out,
        "Running update for all dependency sets (offline, cache-only)\n\
         Would run resolvers for the selected sets\n",
        "{out}"
    );
    let online = Harness::new("update-online-dryrun");
    let (code, out, err) = online.run(&["update", "--apply", "--dry-run"]);
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(
        out,
        "Running update for all dependency sets\nWould run resolvers for the selected sets\n",
        "{out}"
    );
    assert_eq!(err, "", "{err}");
}
