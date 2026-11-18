use super::super::common::{PhaseKind, PhaseRun};
use super::super::quality::execute_quality_phase;
use super::super::test_support::*;
use super::SharedChecks;
use dx_plan::args::{Command, Invocation};
use dx_testing::cli::json_events;
use serde_json::Value;
use std::cell::RefCell;

/// A workspace whose format, lint and typecheck aspects each report their own finding.
fn mixed(name: &str, lint_succeeds: bool) -> (Harness, Vec<(Option<&'static str>, String)>) {
    let harness = umbrella_clean(name);
    let mut events = aspect_events(&harness, FORMAT_ASPECT, "format finding", true);
    events.extend(aspect_events(
        &harness,
        LINT_ASPECT,
        "lint finding",
        lint_succeeds,
    ));
    events.extend(aspect_events(&harness, TYPECHECK_ASPECT, "", true));
    (harness, events)
}

fn umbrella_invocation(harness: &Harness, words: &[&str]) -> Invocation {
    let parsed = dx_plan::args::parse_with_ci(&dx_testing::cli::strings(words), &|_| None, false)
        .expect("parse");
    dx_plan::args::apply_here(&parsed, &harness.workspace, &harness.cwd).expect("workspace")
}

/// Runs every quality phase of `dx check` on its own build, then on one shared build, and
/// returns each phase's run and output both ways along with the builds of each.
#[expect(clippy::type_complexity, reason = "the type is spelled out once")]
fn both_ways(
    harness: &Harness,
    events: &[(Option<&'static str>, String)],
    words: &[&str],
) -> ([Vec<(PhaseRun, String, String)>; 2], [Vec<Vec<String>>; 2]) {
    let inv = umbrella_invocation(harness, words);
    let request = inv.umbrella_request().expect("umbrella request");
    let mut runs = [Vec::new(), Vec::new()];
    let mut builds = [Vec::new(), Vec::new()];
    for (way, shared) in [None, Some(SharedChecks::new(&request))]
        .into_iter()
        .enumerate()
    {
        let mut shared = shared;
        let runner = AspectRunner {
            inner: harness.runner(),
            events: events.to_vec(),
            builds: RefCell::new(Vec::new()),
        };
        for phase in [Command::Format, Command::Lint, Command::Typecheck] {
            let phase_request = request
                .phase_quality_request(phase, Vec::new())
                .expect("phase request");
            let (mut out, mut err) = (Vec::new(), Vec::new());
            let run = execute_quality_phase(
                &phase_request,
                harness.env(&runner, &mut out, &mut err),
                shared.as_mut(),
            );
            runs[way].push((
                run,
                String::from_utf8(out).expect("stdout"),
                String::from_utf8(err).expect("stderr"),
            ));
        }
        drop(shared);
        builds[way] = runner.builds.take();
    }
    (runs, builds)
}

fn leftover_events(harness: &Harness) -> Vec<String> {
    std::fs::read_dir(&harness.temp)
        .expect("temp")
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with("dx-bep-"))
        .collect()
}

#[test]
fn shared_phases_report_what_their_own_builds_report() {
    let (harness, events) = mixed("shared-same-output", true);
    let (runs, builds) = both_ways(&harness, &events, &["check", "--output=json"]);
    assert_eq!(builds[0].len(), 3, "one build per phase on its own");
    assert_eq!(builds[1].len(), 1, "one shared build");
    assert_eq!(runs[1], runs[0]);
    let kinds: Vec<PhaseKind> = runs[1].iter().map(|(run, _, _)| run.kind).collect();
    assert_eq!(
        kinds,
        [PhaseKind::Findings, PhaseKind::Findings, PhaseKind::Clean]
    );
    let messages = |out: &str| -> Vec<String> {
        json_events(out)
            .iter()
            .filter(|event| event["event"] == "diagnostic" || event["event"] == "notice")
            .map(|event| event["message"].as_str().unwrap_or_default().to_owned())
            .collect()
    };
    let format = messages(&runs[1][0].1);
    assert!(format.iter().any(|m| m == "format finding"), "{format:?}");
    assert!(
        format
            .iter()
            .any(|m| m.starts_with("real_format_aspect-tool")),
        "{format:?}"
    );
    assert!(!format.iter().any(|m| m.contains("lint")), "{format:?}");
    assert!(leftover_events(&harness).is_empty());
}

#[test]
fn a_lint_failure_in_the_shared_build_leaves_format_and_typecheck_clean() {
    let (harness, events) = mixed("shared-lint-fails", false);
    let (runs, builds) = both_ways(&harness, &events, &["check", "--output=json"]);
    assert_eq!(builds[1].len(), 1);
    assert_eq!(runs[1], runs[0]);
    let codes: Vec<(i32, PhaseKind)> = runs[1]
        .iter()
        .map(|(run, _, _)| (run.code, run.kind))
        .collect();
    assert_eq!(
        codes,
        [
            (1, PhaseKind::Findings),
            (1, PhaseKind::Tool),
            (0, PhaseKind::Clean)
        ]
    );
    assert!(
        !runs[1][0].1.contains("the tool failed"),
        "format reports no lint failure: {}",
        runs[1][0].1
    );
    assert!(leftover_events(&harness).is_empty());
}

#[test]
fn check_runs_generate_and_one_quality_build_without_a_probe() {
    let mut harness = umbrella_clean("shared-two-builds");
    harness.probe_shards = Some(vec!["real_rust_lint".to_owned()]);
    let (code, out, err) = harness.run(&["check", "--output=json"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(harness.probe_argv.borrow().is_empty());
    let builds = harness.build_argv.borrow();
    assert_eq!(builds.len(), 2, "{builds:?}");
    let aspects = builds[1]
        .iter()
        .find_map(|arg| arg.strip_prefix("--aspects="))
        .expect("aspects");
    for command in [Command::Format, Command::Lint, Command::Typecheck] {
        for aspect in dx_plan::plan::spec(command).aspects {
            assert!(
                aspects
                    .split(',')
                    .any(|passed| dx_bep::aspect_key(passed) == *aspect),
                "{aspect} missing from {aspects}"
            );
        }
    }
    let phases: Vec<Value> = json_events(&out)
        .into_iter()
        .filter(|event| event["event"] == "phase_finished")
        .collect();
    assert_eq!(phases.len(), 4, "{out}");
    assert!(leftover_events(&harness).is_empty());
}

#[test]
fn a_lint_failure_fails_only_lint_in_the_umbrella() {
    let (harness, events) = mixed("shared-umbrella-lint-fails", false);
    let runner = AspectRunner {
        inner: harness.runner(),
        events,
        builds: RefCell::new(Vec::new()),
    };
    let inv = umbrella_invocation(&harness, &["check", "--output=json"]);
    let (code, out, err) = harness.execute_with(&inv, &runner);
    assert_ne!(code, 0, "{out}{err}");
    assert_eq!(runner.builds.borrow().len(), 1);
    assert_eq!(harness.build_argv.borrow().len(), 1, "generate only");
    let exits: Vec<(String, Value)> = json_events(&out)
        .into_iter()
        .filter(|event| event["event"] == "phase_finished")
        .map(|event| {
            (
                event["phase"].as_str().unwrap_or_default().to_owned(),
                event["exit_code"].clone(),
            )
        })
        .collect();
    assert_eq!(
        exits,
        [
            ("generate".to_owned(), Value::from(0)),
            ("format".to_owned(), Value::from(1)),
            ("lint".to_owned(), Value::from(1)),
            ("typecheck".to_owned(), Value::from(0)),
        ],
        "{out}"
    );
    assert!(leftover_events(&harness).is_empty());
}

/// A shared build of format and lint that failed with `failing`, lint passing one aspect format
/// also passes.
fn failed_build(failing: Option<&[Option<&str>]>) -> super::SharedBuild {
    let aspects = |names: &[&str]| names.iter().map(|name| (*name).to_owned()).collect();
    super::SharedBuild {
        bep: std::path::PathBuf::from("unused.json"),
        members: vec![
            (
                Command::Format,
                aspects(&["q:f.bzl%format", "q:f.bzl%both"]),
            ),
            (Command::Lint, aspects(&["q:f.bzl%lint", "q:f.bzl%both"])),
        ],
        bazel_options: Vec::new(),
        resolved: dx_plan::resolve::ResolvedScope {
            scope: dx_process::Scope::Labels(Vec::new()),
            targets: Vec::new(),
            excluded: Vec::new(),
            kept: Vec::new(),
        },
        code: 1,
        failing: failing.map(|failing| {
            failing
                .iter()
                .map(|aspect| aspect.map(str::to_owned))
                .collect()
        }),
        unread: 2,
    }
}

#[test]
fn a_failure_goes_to_every_phase_that_passed_its_aspect() {
    let codes = |failing: Option<&[Option<&str>]>| {
        let build = failed_build(failing);
        (
            build.phase_code(Command::Format),
            build.phase_code(Command::Lint),
        )
    };
    assert_eq!(codes(Some(&[Some("q:f.bzl%lint")])), (0, 1));
    assert_eq!(codes(Some(&[Some("q:f.bzl%format")])), (1, 0));
    assert_eq!(
        codes(Some(&[Some("q:f.bzl%both")])),
        (1, 1),
        "shared aspect"
    );
    assert_eq!(
        codes(Some(&[Some("q:f.bzl%lint"), Some("q:f.bzl%format")])),
        (1, 1),
        "a foreign failure masks no own failure"
    );
    // Failures the events cannot pin on one member belong to every member, so none exits 0.
    assert_eq!(codes(Some(&[None])), (1, 1), "target-level failure");
    assert_eq!(
        codes(Some(&[Some("q:f.bzl%other")])),
        (1, 1),
        "unknown aspect"
    );
    assert_eq!(codes(None), (1, 1), "unreadable events");
}
