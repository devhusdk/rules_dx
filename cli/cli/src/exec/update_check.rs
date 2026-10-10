use super::super::test_support::*;
use super::update_live::*;

#[test]
fn preset_check_dry_run_never_reads_or_writes() {
    for json in [false, true] {
        let harness = Harness::new("update-check-dry");
        let output = if json {
            "--output=json"
        } else {
            "--output=text"
        };
        let (code, out, err) = harness.run(&["update", "--check", "--dry-run", output]);
        assert_eq!(code, 0, "{out}{err}");
        assert!(harness.seen_env.borrow().is_empty());
        assert!(!harness.workspace.join("tools").exists());
        if json {
            let events: Vec<serde_json::Value> = out
                .lines()
                .map(|line| serde_json::from_str(line).expect("event"))
                .collect();
            assert_eq!(events.len(), 2);
            assert_eq!(events[0]["dry_run"], true);
            assert_eq!(events[0]["mode"], serde_json::json!("check"));
            assert_eq!(events[1]["exit_code"], 0);
        } else {
            assert!(
                out.contains("Running update --check for all dependency sets"),
                "{out}"
            );
            assert!(
                out.contains(
                    "Would check uv: uv lock --check --directory python/tests/fixtures/hello"
                ),
                "{out}"
            );
            assert!(out.contains("Would leave go pinned"), "{out}");
        }
    }
}

#[test]
fn updater_spawn_and_signal_failures_keep_other_sets_independent() {
    for spawn_error in [false, true] {
        let mut runner = ScriptRunner::new(&[("cargo", None)]);
        runner.io_error = spawn_error;
        let (code, out, err) = run_with(&["update", "cargo", "go", "--output=json"], &runner);
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

fn run_probe(harness: &Harness, argv: &[&str], codes: &[Option<i32>]) -> ProbeRun {
    let words: Vec<String> = argv.iter().map(|word| (*word).to_owned()).collect();
    let parsed = crate::args::parse(&words).expect("parse");
    let invocation =
        crate::args::apply_here(&parsed, &harness.workspace, &harness.cwd).expect("scopes");
    harness.probe_with(&invocation, codes)
}

#[test]
fn check_selectors_resolve_to_sets_not_preset() {
    let runner = ScriptRunner::new(&[]);
    let (code, out, err) = run_with(&["update", "--check", "cargo"], &runner);
    assert_eq!(code, 1, "{out}{err}");
    assert!(err.contains("update_set_unsupported"), "{err}");
    assert!(err.contains("cannot check cargo"), "{err}");
    assert!(!out.contains("preset"), "{out}");
    assert!(!err.contains("preset"), "{err}");
    assert!(
        runner.calls.borrow().is_empty(),
        "unavailable launches nothing"
    );

    let runner = ScriptRunner::new(&[]);
    let (code, out, err) = run_with(&["update", "--check", "uv"], &runner);
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(runner.calls.borrow().len(), 1);
    assert_eq!(
        runner.calls.borrow()[0],
        vec![
            "uv".to_owned(),
            "lock".to_owned(),
            "--check".to_owned(),
            "--directory".to_owned(),
            "python/tests/fixtures/hello".to_owned(),
        ]
    );
    assert!(
        out.contains("uv lockfile current (python/tests/fixtures/hello/uv.lock)"),
        "{out}"
    );
}

#[test]
fn check_ignores_stale_preset() {
    let harness = Harness::new("update-check-ignores-preset");
    harness.write_source("tools/bazelrc/preset.bazelrc", "# dirty\n");
    let run = run_probe(&harness, &["update", "--check", "uv"], &[Some(0)]);
    assert_eq!(run.code, 0, "{}", run.out);
    assert_eq!(run.argv.len(), 1);
    assert_eq!(
        run.argv[0],
        vec![
            "uv".to_owned(),
            "lock".to_owned(),
            "--check".to_owned(),
            "--directory".to_owned(),
            "python/tests/fixtures/hello".to_owned(),
        ]
    );
    assert!(
        run.out.contains("Running update --check for uv"),
        "{}",
        run.out
    );
    assert_eq!(
        std::fs::read_to_string(harness.workspace.join("tools/bazelrc/preset.bazelrc"))
            .expect("read"),
        "# dirty\n"
    );
}

#[test]
fn check_current_reports_lockfile_without_writing() {
    let harness = Harness::new("update-check-current");
    harness.write_source("python/tests/fixtures/hello/uv.lock", "lock-bytes\n");
    let run = run_probe(&harness, &["update", "--check", "uv"], &[Some(0)]);
    assert_eq!(run.code, 0, "{}", run.out);
    assert_eq!(run.argv.len(), 1, "only the selected set runs");
    assert!(
        run.out
            .contains("uv lockfile current (python/tests/fixtures/hello/uv.lock)"),
        "{}",
        run.out
    );
    assert_eq!(
        std::fs::read_to_string(
            harness
                .workspace
                .join("python/tests/fixtures/hello/uv.lock")
        )
        .expect("read"),
        "lock-bytes\n"
    );
}

#[test]
fn check_stale_check_failure_reports_without_mutation() {
    let mut harness = Harness::new("update-check-stale");
    harness.bazel_code = 1;
    harness.write_source("python/tests/fixtures/hello/uv.lock", "lock-bytes\n");
    let (code, out, err) = harness.run(&["update", "--check", "uv"]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(err.contains("failed to check uv"), "{err}");
    assert!(err.contains("updater exited 1"), "{err}");
    assert_eq!(
        std::fs::read_to_string(
            harness
                .workspace
                .join("python/tests/fixtures/hello/uv.lock")
        )
        .expect("read"),
        "lock-bytes\n"
    );
}

#[test]
fn check_unavailable_names_refresh_command() {
    let runner = ScriptRunner::new(&[]);
    let (code, out, err) = run_with(&["update", "--check", "npm"], &runner);
    assert_eq!(code, 1, "{out}{err}");
    assert!(err.contains("update_set_unsupported"), "{err}");
    assert!(err.contains("cannot check npm"), "{err}");
    assert!(err.contains("refresh with `dx update npm`"), "{err}");
    assert!(
        runner.calls.borrow().is_empty(),
        "unavailable launches nothing"
    );
}

#[test]
fn check_pinned_sets_report_manual_without_launching() {
    for set in ["go", "ruby", "powershell"] {
        let runner = ScriptRunner::new(&[]);
        let (code, out, err) = run_with(&["update", "--check", set], &runner);
        assert_eq!(code, 0, "{out}{err}");
        assert!(out.contains(&format!("{set} pins are manual")), "{out}");
        assert!(out.contains("nothing to resolve"), "{out}");
        assert!(runner.calls.borrow().is_empty(), "pinned launches nothing");
    }
}

#[test]
fn check_json_reports_current_pinned_unsupported() {
    let runner = ScriptRunner::new(&[]);
    let (code, out, err) = run_with(
        &["update", "--check", "uv", "go", "npm", "--output=json"],
        &runner,
    );
    assert_eq!(code, 1, "{out}{err}");
    let events = json_events(&out);
    let kinds = event_kinds(&events);
    assert_eq!(kinds[0], "command_started");
    assert_eq!(kinds[kinds.len() - 1], "command_finished");
    let by_code = |code: &str| {
        events
            .iter()
            .find(|event| event.get("code").and_then(|c| c.as_str()) == Some(code))
            .unwrap_or_else(|| panic!("{code}: {out}"))
            .clone()
    };
    let current = by_code("update_set_current");
    assert_eq!(current["scope"], serde_json::json!(["uv"]));
    assert_eq!(current["correlation"], serde_json::json!("update:uv"));
    let pinned = by_code("update_set_pinned");
    assert_eq!(pinned["scope"], serde_json::json!(["go"]));
    let unsupported = by_code("update_set_unsupported");
    assert_eq!(unsupported["correlation"], serde_json::json!("update:npm"));
    assert!(
        unsupported["message"]
            .as_str()
            .expect("message")
            .contains("refresh with `dx update npm`"),
        "{out}"
    );
    let recovery = by_code("update_recovery");
    assert!(
        recovery["message"]
            .as_str()
            .expect("message")
            .contains("dx update npm"),
        "{out}"
    );
    assert_eq!(
        events.last().expect("finished")["exit_code"],
        serde_json::json!(1)
    );
    assert_eq!(runner.calls.borrow().len(), 1, "only uv launches");
}

#[test]
fn check_rejects_unknown_selector() {
    let harness = Harness::new("update-check-unknown");
    let (code, _, err) = harness.run(&["update", "--check", "crates"]);
    assert_eq!(code, 2, "{err}");
}

#[test]
fn default_update_leaves_preset_alone() {
    let harness = Harness::new("update-default-preset");
    harness.write_source(
        ".bazelrc",
        "import %workspace%/tools/bazelrc/preset.bazelrc\ntry-import %workspace%/user.bazelrc\n",
    );
    harness.write_source("tools/bazelrc/preset.bazelrc", "# dirty\n");
    let (code, out, err) = harness.run(&["update", "go"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(!out.contains("preset"), "{out}");
    assert!(out.contains("go pins are manual"), "{out}");
    assert_eq!(
        std::fs::read_to_string(harness.workspace.join("tools/bazelrc/preset.bazelrc"))
            .expect("read"),
        "# dirty\n"
    );
    assert_eq!(err, "", "{err}");
}

#[test]
fn update_json_never_emits_change_or_mutation() {
    let runner = ScriptRunner::new(&[]);
    let (code, out, err) = run_with(&["update", "--output=json"], &runner);
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
fn update_json_completeness_is_per_set_plus_finished() {
    let runner = ScriptRunner::new(&[("maven", Some(1))]);
    let (code, out, err) = run_with(&["update", "--output=json"], &runner);
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
    assert!(recovery_message.contains("dx update maven"), "{out}");
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
fn update_json_check_current_and_dryrun_emit_no_file_events_or_counts() {
    let harness = Harness::new("update-586-check-json");
    harness.write_source("python/tests/fixtures/hello/uv.lock", "lock-bytes\n");
    let (code, out, err) = harness.run(&["update", "--check", "uv", "--output=json"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(!out.contains("\"event\":\"change\""), "{out}");
    assert!(!out.contains("\"event\":\"mutation\""), "{out}");
    let events = json_events(&out);
    assert!(
        events
            .iter()
            .any(|event| event["code"] == serde_json::json!("update_set_current")),
        "{out}"
    );
    let finished = events.last().expect("finished");
    assert!(finished.get("changes").is_none(), "{out}");
    assert!(finished.get("mutations").is_none(), "{out}");
    assert!(finished.get("diagnostics").is_none(), "{out}");
    assert_eq!(
        std::fs::read_to_string(
            harness
                .workspace
                .join("python/tests/fixtures/hello/uv.lock")
        )
        .expect("read"),
        "lock-bytes\n"
    );

    let dry = Harness::new("update-586-dryrun-json");
    let (code, out, err) = dry.run(&["update", "--dry-run", "--output=json"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(!out.contains("\"event\":\"change\""), "{out}");
    assert!(!out.contains("\"event\":\"mutation\""), "{out}");
}

#[test]
fn update_failure_reports_recovery_in_text_and_json() {
    let runner = ScriptRunner::new(&[("maven", Some(1))]);
    let (code, out, err) = run_with(&["update"], &runner);
    assert_eq!(code, 1, "{out}{err}");
    assert!(err.contains("update_recovery"), "{err}");
    assert!(err.contains("dx update maven"), "{err}");
    assert!(err.contains("idempotent"), "{err}");
    assert!(err.contains("git checkout --"), "{err}");

    let runner = ScriptRunner::new(&[("maven", Some(1))]);
    let (code, out, err) = run_with(&["update", "--output=json"], &runner);
    assert_eq!(code, 1, "{out}{err}");
    assert!(out.contains("\"code\":\"update_recovery\""), "{out}");
    assert!(out.contains("dx update maven"), "{out}");
    assert!(err.contains("update_recovery"), "{err}");
}

#[test]
fn update_success_emits_no_recovery() {
    let runner = ScriptRunner::new(&[]);
    let (code, out, err) = run_with(&["update"], &runner);
    assert_eq!(code, 0, "{out}{err}");
    assert!(!err.contains("update_recovery"), "{err}");
    assert!(!out.contains("update_recovery"), "{out}");

    let runner = ScriptRunner::new(&[]);
    let (code, out, err) = run_with(&["update", "--output=json"], &runner);
    assert_eq!(code, 0, "{out}{err}");
    assert!(!out.contains("update_recovery"), "{out}");
}

#[test]
fn offline_dry_run_plans_cache_only_without_launching() {
    fn offline_line(set: &str) -> String {
        format!(
            "Cannot update {set}: offline_required: cannot update {set} without network (re-run without --offline once connected)"
        )
    }
    fn pinned_line(set: &str) -> String {
        format!("Would leave {set} pinned (manual pins; nothing to resolve)")
    }
    let expected = format!(
        "{}\n{}\n",
        "Running update for all dependency sets (offline, cache-only)",
        [
            offline_line("cargo"),
            pinned_line("go"),
            offline_line("maven"),
            offline_line("npm"),
            offline_line("npm-adopt"),
            offline_line("npm-adopt-polyglot"),
            offline_line("npm-tools"),
            offline_line("nuget"),
            pinned_line("powershell"),
            pinned_line("ruby"),
            offline_line("uv"),
            offline_line("uv-adopt"),
            offline_line("uv-adopt-polyglot"),
            offline_line("uv-tools"),
        ]
        .join("\n")
    );
    let harness = Harness::new("update-offline-dryrun");
    let (code, out, err) = harness.run(&["update", "--offline", "--dry-run"]);
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(out, expected, "{out}");
    assert_eq!(err, "", "{err}");
    assert!(
        harness.seen_env.borrow().is_empty(),
        "offline dry-run launches nothing"
    );
    let online = Harness::new("update-online-dryrun");
    let (code, out, err) = online.run(&["update", "--dry-run"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(
        out.starts_with("Running update for all dependency sets\n"),
        "{out}"
    );
    assert!(
        out.contains("Would update cargo: bazel build //rust/tests/fixtures/hello:hello"),
        "{out}"
    );
    assert!(
        out.contains("Would update maven: bazel run @maven//:pin"),
        "{out}"
    );
    assert!(
        out.contains("Would update npm: bazel run @pnpm//:pnpm -- --dir"),
        "{out}"
    );
    assert!(
        out.contains("Would update uv: uv lock --directory python/tests/fixtures/hello"),
        "{out}"
    );
    assert!(out.contains("Would leave go pinned"), "{out}");
    assert_eq!(out.lines().count(), 15, "{out}");
    assert_eq!(err, "", "{err}");
}

#[test]
fn frozen_dry_run_locks_resolution_without_launching() {
    fn frozen_line(set: &str) -> String {
        format!(
            "Cannot update {set}: frozen_locked: cannot change {set} resolution while frozen (re-run without --frozen to allow resolver changes)"
        )
    }
    fn pinned_line(set: &str) -> String {
        format!("Would leave {set} pinned (manual pins; nothing to resolve)")
    }
    let expected = format!(
        "{}\n{}\n",
        "Running update for all dependency sets (frozen, no resolution changes)",
        [
            frozen_line("cargo"),
            pinned_line("go"),
            frozen_line("maven"),
            frozen_line("npm"),
            frozen_line("npm-adopt"),
            frozen_line("npm-adopt-polyglot"),
            frozen_line("npm-tools"),
            frozen_line("nuget"),
            pinned_line("powershell"),
            pinned_line("ruby"),
            frozen_line("uv"),
            frozen_line("uv-adopt"),
            frozen_line("uv-adopt-polyglot"),
            frozen_line("uv-tools"),
        ]
        .join("\n")
    );
    let harness = Harness::new("update-frozen-dryrun");
    let (code, out, err) = harness.run(&["update", "--frozen", "--dry-run"]);
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(out, expected, "{out}");
    assert_eq!(err, "", "{err}");
    assert!(
        harness.seen_env.borrow().is_empty(),
        "frozen dry-run launches nothing"
    );
    assert!(
        !out.contains("offline"),
        "frozen no longer implies offline: {out}"
    );
}

#[test]
fn frozen_live_fails_with_frozen_locked_without_launching() {
    let runner = ScriptRunner::new(&[]);
    let (code, out, err) = run_with(&["update", "cargo", "--frozen"], &runner);
    assert_eq!(code, 1, "{out}{err}");
    assert!(err.contains("frozen_locked"), "{err}");
    assert!(err.contains("cannot change cargo resolution"), "{err}");
    assert!(runner.calls.borrow().is_empty(), "frozen launches nothing");
    let (code, out, err) = run_with(&["update", "cargo", "--frozen", "--output=json"], &runner);
    assert_eq!(code, 1, "{out}{err}");
    assert!(out.contains("\"code\":\"frozen_locked\""), "{out}");
    assert!(!out.contains("\"code\":\"update_failed\""), "{out}");
    assert!(!out.contains("\"code\":\"offline_required\""), "{out}");
    let go_runner = ScriptRunner::new(&[]);
    let (code, out, err) = run_with(&["update", "go", "--frozen"], &go_runner);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("go pins are manual"), "{out}");
    assert!(out.contains("nothing to resolve"), "{out}");
    assert!(
        go_runner.calls.borrow().is_empty(),
        "go pinned launches nothing"
    );
}

#[test]
fn frozen_locked_code_is_stable_single_source() {
    assert_eq!(
        crate::exec::common::CODE_FROZEN_LOCKED,
        "frozen_locked"
    );
}
    let runner = ScriptRunner::new(&[]);
    let (code, out, err) = run_with(&["update", "cargo", "--offline"], &runner);
    assert_eq!(code, 1, "{out}{err}");
    assert!(err.contains("offline_required"), "{err}");
    assert!(err.contains("cannot update cargo without network"), "{err}");
    assert!(runner.calls.borrow().is_empty(), "offline launches nothing");
    let (code, out, err) = run_with(&["update", "cargo", "--offline", "--output=json"], &runner);
    assert_eq!(code, 1, "{out}{err}");
    assert!(out.contains("\"code\":\"offline_required\""), "{out}");
    assert!(!out.contains("\"code\":\"update_failed\""), "{out}");
    let go_runner = ScriptRunner::new(&[]);
    let (code, out, err) = run_with(&["update", "go", "--offline"], &go_runner);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("go pins are manual"), "{out}");
    assert!(out.contains("nothing to resolve"), "{out}");
    assert!(
        go_runner.calls.borrow().is_empty(),
        "go pinned launches nothing"
    );
}

#[test]
fn offline_required_code_is_stable_single_source() {
    assert_eq!(
        crate::exec::common::CODE_OFFLINE_REQUIRED,
        "offline_required"
    );
}
