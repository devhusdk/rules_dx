use super::super::test_support::*;
use super::update_live::*;
use crate::test_support::strings;

#[test]
fn bare_update_checks_instead_of_applying() {
    for argv in [
        &["update", "cargo"][..],
        &["update", "--check", "cargo"][..],
    ] {
        let runner = ScriptRunner::new(&[]);
        let (code, out, err) = run_with(argv, &runner);
        assert_eq!(code, 0, "{argv:?}: {out}{err}");
        assert!(out.contains("Checking cargo"), "{argv:?}: {out}");
        assert!(
            out.contains("no lock-freshness check is implemented for cargo"),
            "{argv:?}: {out}"
        );
        assert!(out.contains("dx update --apply cargo"), "{argv:?}: {out}");
        assert!(
            runner.calls.borrow().is_empty(),
            "{argv:?} launches nothing"
        );
    }
}

#[test]
fn check_and_apply_share_selector_resolution() {
    let check_runner = ScriptRunner::new(&[]);
    let (code, out, err) = run_with(&["update", "npm:jest"], &check_runner);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("Checking npm:jest"), "{out}");
    assert!(out.contains("unavailable"), "{out}");
    assert!(check_runner.calls.borrow().is_empty());

    let apply_runner = ScriptRunner::new(&[]);
    let (code, out, err) = run_with(&["update", "--apply", "npm:jest"], &apply_runner);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("Running update for npm:jest"), "{out}");
    assert_eq!(apply_runner.calls.borrow().len(), 1);
    assert!(apply_runner.calls.borrow()[0].contains(&"jest".to_owned()));
}

#[test]
fn check_current_reports_locks_without_writing() {
    let runner = ScriptRunner::new(&[]);
    let (code, out, err) = run_with(&["update", "--check", "uv"], &runner);
    assert_eq!(code, 0, "{out}{err}");
    assert!(
        out.contains("uv is current (python/tests/fixtures/hello/uv.lock)"),
        "{out}"
    );
    assert_eq!(runner.calls.borrow().len(), 1);
    let argv = &runner.calls.borrow()[0];
    assert!(argv.contains(&"--check".to_owned()), "{argv:?}");
    assert_eq!(err, "", "{err}");
}

#[test]
fn check_runs_the_qualified_resolver_with_lock_scope() {
    let runner = ScriptRunner::new(&[]);
    let (code, out, err) = run_with(&["update", "--check", "uv-tools"], &runner);
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(runner.calls.borrow().len(), 1);
    let argv = runner.calls.borrow()[0].clone();
    assert_eq!(argv[0], "uv", "{argv:?}");
    assert_eq!(argv[1], "lock", "{argv:?}");
    assert!(argv.contains(&"--check".to_owned()), "{argv:?}");
    let at = argv
        .iter()
        .position(|arg| arg == "--directory")
        .expect("--directory");
    assert_eq!(argv[at + 1], "quality/tools/python", "{argv:?}");
    assert!(out.contains("uv-tools is current"), "{out}");
}

#[test]
fn check_stale_fails_with_apply_hint() {
    let runner = ScriptRunner::new(&[("uv", Some(1))]);
    let (code, out, err) = run_with(&["update", "--check", "uv"], &runner);
    assert_eq!(code, 1, "{out}{err}");
    assert!(err.contains("update_failed"), "{err}");
    assert!(err.contains("uv is stale"), "{err}");
    assert!(err.contains("dx update --apply uv"), "{err}");
    assert_eq!(runner.calls.borrow().len(), 1);
}

#[test]
fn check_stale_leaves_locks_untouched() {
    let harness = Harness::new("update-check-stale-lock");
    harness.write_source("python/tests/fixtures/hello/uv.lock", "# checked-in lock\n");
    let invocation = crate::args::parse(&strings(&["update", "--check", "uv"])).expect("parse");
    let runner = ScriptRunner::new(&[("uv", Some(1))]);
    let (code, _, err) = harness.execute_with(&invocation, &runner);
    assert_eq!(code, 1, "{err}");
    assert_eq!(
        std::fs::read_to_string(
            harness
                .workspace
                .join("python/tests/fixtures/hello/uv.lock")
        )
        .expect("read"),
        "# checked-in lock\n"
    );
}

#[test]
fn check_missing_lock_fails_like_stale_without_creating_it() {
    let harness = Harness::new("update-check-missing-lock");
    let invocation = crate::args::parse(&strings(&["update", "--check", "uv"])).expect("parse");
    let runner = ScriptRunner::new(&[("uv", Some(2))]);
    let (code, _, err) = harness.execute_with(&invocation, &runner);
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("uv is stale"), "{err}");
    assert!(err.contains("dx update --apply uv"), "{err}");
    assert!(
        !harness
            .workspace
            .join("python/tests/fixtures/hello/uv.lock")
            .is_file(),
        "check must not create a missing lock"
    );
    assert_eq!(runner.calls.borrow().len(), 1);
}

#[test]
fn check_unavailable_names_locks_and_apply() {
    let runner = ScriptRunner::new(&[]);
    let (code, out, err) = run_with(&["update", "--check", "cargo", "npm"], &runner);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("Checking cargo, npm"), "{out}");
    assert!(
        out.contains("no lock-freshness check is implemented for cargo"),
        "{out}"
    );
    assert!(
        out.contains("rust/tests/fixtures/hello/Cargo.lock"),
        "{out}"
    );
    assert!(out.contains("cargo-bazel-lock.json"), "{out}");
    assert!(
        out.contains("no lock-freshness check is implemented for npm"),
        "{out}"
    );
    assert!(out.contains("pnpm-lock.yaml"), "{out}");
    assert!(runner.calls.borrow().is_empty());
    assert!(out.contains("2 unavailable"), "{out}");
}

#[test]
fn check_all_reports_per_set_with_honest_counts() {
    let runner = ScriptRunner::new(&[]);
    let (code, out, err) = run_with(&["update", "--check"], &runner);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("Checking all dependency sets"), "{out}");
    assert!(out.contains("uv is current"), "{out}");
    assert!(
        out.contains("is unverified") || out.contains("unavailable"),
        "{out}"
    );
    assert!(
        out.contains("4 succeeded, 0 failed, 0 blocked, 10 unavailable"),
        "{out}"
    );
    assert_eq!(runner.calls.borrow().len(), 4);
}

#[test]
fn check_stale_and_unavailable_stay_independent() {
    let runner = ScriptRunner::new(&[("uv", Some(1))]);
    let (code, out, err) = run_with(&["update", "--check", "uv", "cargo"], &runner);
    assert_eq!(code, 1, "{out}{err}");
    assert!(err.contains("uv is stale"), "{err}");
    assert!(err.contains("update_recovery"), "{err}");
    assert!(err.contains("dx update --apply uv"), "{err}");
    assert!(
        out.contains("no lock-freshness check is implemented for cargo"),
        "{out}"
    );
    assert!(
        out.contains("0 succeeded, 1 failed, 0 blocked, 1 unavailable"),
        "{out}"
    );
}

#[test]
fn check_lookup_and_signal_failures_are_failed_not_current() {
    for signalled in [false, true] {
        let mut runner = ScriptRunner::new(&[("uv", None)]);
        runner.io_error = !signalled;
        let (code, out, err) = run_with(&["update", "--check", "uv"], &runner);
        assert_eq!(code, 1, "{out}{err}");
        assert!(!out.contains("uv is current"), "{out}");
        assert!(err.contains("failed to check uv"), "{err}");
        if signalled {
            assert!(err.contains("terminated by signal"), "{err}");
        } else {
            assert!(err.contains("failed to launch checker"), "{err}");
        }
    }
}

#[test]
fn check_offline_fails_closed_without_launching() {
    let runner = ScriptRunner::new(&[]);
    let (code, out, err) = run_with(&["update", "--check", "uv", "--offline"], &runner);
    assert_eq!(code, 1, "{out}{err}");
    assert!(err.contains("offline_required"), "{err}");
    assert!(err.contains("cannot update uv without network"), "{err}");
    assert!(runner.calls.borrow().is_empty());
    let pinned = ScriptRunner::new(&[]);
    let (code, out, err) = run_with(&["update", "--check", "go", "--offline"], &pinned);
    assert_eq!(code, 0, "{out}{err}");
    assert!(
        out.contains("no lock-freshness check is implemented for go"),
        "{out}"
    );
    assert!(pinned.calls.borrow().is_empty());
}

#[test]
fn check_rejects_apply_together() {
    let harness = Harness::new("update-check-apply-conflict");
    let (code, _, err) = harness.run(&["update", "--check", "--apply", "uv"]);
    assert_eq!(code, 2, "{err}");
    assert!(err.contains("--check and --apply"), "{err}");
    assert!(err.contains("mutually exclusive"), "{err}");
}

#[test]
fn check_never_touches_preset() {
    let harness = Harness::new("update-check-preset");
    harness.write_source(
        ".bazelrc",
        "import %workspace%/tools/bazelrc/preset.bazelrc\ntry-import %workspace%/user.bazelrc\n",
    );
    harness.write_source("tools/bazelrc/preset.bazelrc", "# dirty\n");
    let (code, out, err) = harness.run(&["update", "--check", "cargo"]);
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(
        std::fs::read_to_string(harness.workspace.join("tools/bazelrc/preset.bazelrc"))
            .expect("read"),
        "# dirty\n"
    );
    assert_eq!(
        std::fs::read_to_string(harness.workspace.join(".bazelrc")).expect("read"),
        "import %workspace%/tools/bazelrc/preset.bazelrc\ntry-import %workspace%/user.bazelrc\n"
    );
}

#[test]
fn apply_never_touches_preset() {
    let harness = Harness::new("update-apply-preset");
    harness.write_source(
        ".bazelrc",
        "import %workspace%/tools/bazelrc/preset.bazelrc\ntry-import %workspace%/user.bazelrc\n",
    );
    harness.write_source("tools/bazelrc/preset.bazelrc", "# dirty\n");
    let invocation = crate::args::parse(&strings(&["update", "--apply", "go"])).expect("parse");
    let runner = ScriptRunner::new(&[]);
    let (code, out, err) = harness.execute_with(&invocation, &runner);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("go is pinned"), "{out}");
    assert_eq!(
        std::fs::read_to_string(harness.workspace.join("tools/bazelrc/preset.bazelrc"))
            .expect("read"),
        "# dirty\n"
    );
    assert!(runner.calls.borrow().is_empty());
}

#[test]
fn check_dry_run_plans_without_launching() {
    let harness = Harness::new("update-check-dry");
    let (code, out, err) = harness.run(&["update", "--check", "--dry-run"]);
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(
        out, "Checking all dependency sets\nWould check without writing anything\n",
        "{out}"
    );
    assert_eq!(err, "", "{err}");
    assert!(harness.seen_env.borrow().is_empty());
    let selected = Harness::new("update-check-dry-selected");
    let (code, out, err) = selected.run(&["update", "uv", "--dry-run"]);
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(
        out, "Checking uv\nWould check without writing anything\n",
        "{out}"
    );
}

#[test]
fn check_dry_run_json_stays_two_events() {
    let harness = Harness::new("update-check-dry-json");
    let (code, out, err) = harness.run(&["update", "--check", "--dry-run", "--output=json"]);
    assert_eq!(code, 0, "{out}{err}");
    let events: Vec<serde_json::Value> = out
        .lines()
        .map(|line| serde_json::from_str(line).expect("event"))
        .collect();
    assert_eq!(events.len(), 2);
    assert_eq!(events[0]["dry_run"], true);
    assert_eq!(events[0]["mode"], serde_json::json!("check"));
    assert_eq!(events[1]["exit_code"], 0);
}

#[test]
fn check_json_reports_current_and_unavailable() {
    let runner = ScriptRunner::new(&[]);
    let (code, out, err) = run_with(
        &["update", "--check", "uv", "cargo", "--output=json"],
        &runner,
    );
    assert_eq!(code, 0, "{out}{err}");
    let events = json_events(&out);
    let kinds = event_kinds(&events);
    assert_eq!(kinds[0], "command_started");
    assert_eq!(kinds[kinds.len() - 1], "command_finished");
    assert!(kinds.contains(&"notice"), "{out}");
    assert!(!out.contains("\"event\":\"change\""), "{out}");
    assert!(!out.contains("\"event\":\"mutation\""), "{out}");
    let codes: Vec<&str> = events
        .iter()
        .filter_map(|event| event.get("code").and_then(|code| code.as_str()))
        .collect();
    assert!(codes.contains(&"update_set_current"), "{out}");
    assert!(codes.contains(&"update_set_unavailable"), "{out}");
    assert!(out.contains("\"correlation\":\"update:uv\""), "{out}");
    assert!(out.contains("\"correlation\":\"update:cargo\""), "{out}");
    assert_eq!(
        events.last().expect("finished")["exit_code"],
        serde_json::json!(0)
    );
}

#[test]
fn check_json_stale_reports_failed_and_recovery() {
    let runner = ScriptRunner::new(&[("uv", Some(1))]);
    let (code, out, err) = run_with(&["update", "--check", "uv", "--output=json"], &runner);
    assert_eq!(code, 1, "{out}{err}");
    assert!(out.contains("\"code\":\"update_failed\""), "{out}");
    assert!(out.contains("uv is stale"), "{out}");
    assert!(out.contains("\"code\":\"update_recovery\""), "{out}");
    assert!(out.contains("dx update --apply uv"), "{out}");
    let events = json_events(&out);
    assert_eq!(
        events.last().expect("finished")["exit_code"],
        serde_json::json!(1)
    );
}

#[test]
fn unsupported_selective_fails_with_its_own_code() {
    let runner = ScriptRunner::new(&[]);
    let (code, out, err) = run_with(&["update", "--check", "cargo:anyhow"], &runner);
    assert_eq!(code, 1, "{out}{err}");
    assert!(err.contains("update_unsupported"), "{err}");
    assert!(err.contains("unsupported update"), "{err}");
    assert!(runner.calls.borrow().is_empty());
    let (code, out, err) = run_with(
        &["update", "--check", "cargo:anyhow", "--output=json"],
        &runner,
    );
    assert_eq!(code, 1, "{out}{err}");
    assert!(out.contains("\"code\":\"update_unsupported\""), "{out}");
    assert!(!out.contains("\"code\":\"update_failed\""), "{out}");
}
