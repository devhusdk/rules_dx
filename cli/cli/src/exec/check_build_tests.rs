use super::test_support::*;
use dx_exec_core::common::{PhaseKind, PhaseRun};
use dx_exec_quality::check_build::SharedChecks;
use dx_exec_quality::execute_quality_phase;
use dx_plan::args::{Command, Invocation};
use dx_testing::cli::json_events;
use serde_json::Value;
use std::cell::RefCell;

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

type BothWays = ([Vec<(PhaseRun, String, String)>; 2], [Vec<Vec<String>>; 2]);

fn both_ways(
    harness: &Harness,
    events: &[(Option<&'static str>, String)],
    words: &[&str],
) -> BothWays {
    both_ways_for(harness, events, &umbrella_invocation(harness, words))
}

fn both_ways_for(
    harness: &Harness,
    events: &[(Option<&'static str>, String)],
    inv: &Invocation,
) -> BothWays {
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
        .filter(|name| name.starts_with("dx-bep-") && !name.contains(".json.shards"))
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

const FORMAT_TARGET: &str = "//a:f";
const LINT_TARGET: &str = "//a:l";
const TYPECHECK_TARGET: &str = "//a:t";

fn affected_scopes(harness: &Harness) -> Invocation {
    let mut inv = umbrella_invocation(harness, &["check", "--output=json"]);
    inv.targets = dx_testing::cli::strings(&[FORMAT_TARGET, LINT_TARGET, TYPECHECK_TARGET]);
    inv.affected_handoff = Some(dx_plan::args::AffectedHandoff {
        format: dx_testing::cli::strings(&[FORMAT_TARGET]),
        lint: dx_testing::cli::strings(&[FORMAT_TARGET, LINT_TARGET]),
        ..Default::default()
    });
    inv
}

fn every_target_events(
    harness: &Harness,
    failing: &[(&str, &'static str)],
) -> Vec<(Option<&'static str>, String)> {
    let mut events = Vec::new();
    for label in [FORMAT_TARGET, LINT_TARGET, TYPECHECK_TARGET] {
        for (aspect, tool) in [
            (FORMAT_ASPECT, "format"),
            (LINT_ASPECT, "lint"),
            (TYPECHECK_ASPECT, "typecheck"),
        ] {
            let finding = format!("{tool} finding on {label}");
            let success = !failing.contains(&(label, aspect));
            events.extend(target_aspect_events(
                harness, label, aspect, &finding, success,
            ));
        }
    }
    events
}

fn finding_messages(out: &str) -> Vec<String> {
    let mut messages: Vec<String> = json_events(out)
        .iter()
        .filter(|event| event["event"] == "diagnostic")
        .map(|event| event["message"].as_str().unwrap_or_default().to_owned())
        .collect();
    messages.sort();
    messages
}

#[test]
fn phases_with_different_targets_share_one_probe_and_one_build_over_their_union() {
    let harness = umbrella_clean("shared-affected-union");
    let events = every_target_events(&harness, &[]);
    let (runs, builds) = both_ways_for(&harness, &events, &affected_scopes(&harness));
    let scopes: Vec<Vec<String>> = builds[0].iter().map(|argv| build_labels(argv)).collect();
    assert_eq!(
        scopes,
        [
            vec![FORMAT_TARGET],
            vec![FORMAT_TARGET, LINT_TARGET],
            vec![FORMAT_TARGET, LINT_TARGET, TYPECHECK_TARGET],
        ],
        "one build per phase on its own"
    );
    assert_eq!(builds[1].len(), 1, "one shared build: {:?}", builds[1]);
    assert_eq!(
        build_labels(&builds[1][0]),
        [FORMAT_TARGET, LINT_TARGET, TYPECHECK_TARGET]
    );
    let probes = harness.probe_argv.borrow();
    assert_eq!(probes.len(), 4, "three probes on their own, one shared");
    assert_eq!(
        build_labels(&probes[3]),
        [FORMAT_TARGET, LINT_TARGET, TYPECHECK_TARGET]
    );
    assert_eq!(runs[1], runs[0]);
    assert_eq!(finding_messages(&runs[1][0].1), ["format finding on //a:f"]);
    assert_eq!(
        finding_messages(&runs[1][1].1),
        ["lint finding on //a:f", "lint finding on //a:l"]
    );
    assert!(leftover_events(&harness).is_empty());
}

#[test]
fn a_failure_on_a_target_only_lint_selected_fails_only_lint() {
    let harness = umbrella_clean("shared-affected-lint-fails");
    let failing = [
        (LINT_TARGET, LINT_ASPECT),
        (TYPECHECK_TARGET, FORMAT_ASPECT),
    ];
    let events = every_target_events(&harness, &failing);
    let (runs, builds) = both_ways_for(&harness, &events, &affected_scopes(&harness));
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
            (1, PhaseKind::Findings)
        ]
    );
    let failed_targets = |out: &str| -> Vec<String> {
        json_events(out)
            .iter()
            .filter(|event| event["event"] == "build_failure")
            .map(|event| event["target"].as_str().unwrap_or_default().to_owned())
            .collect()
    };
    assert!(failed_targets(&runs[1][0].1).is_empty(), "{}", runs[1][0].1);
    let lint = failed_targets(&runs[1][1].1);
    assert!(!lint.is_empty(), "{}", runs[1][1].1);
    assert!(lint.iter().all(|target| target == LINT_TARGET), "{lint:?}");
    assert!(leftover_events(&harness).is_empty());
}

#[test]
fn check_runs_generate_one_shard_probe_and_one_quality_build_over_the_probed_shards() {
    let mut harness = umbrella_clean("shared-two-builds");
    harness.probe_shards = Some(vec!["real_rust_lint".to_owned()]);
    let (code, out, err) = harness.run(&["check", "--output=json"]);
    assert_eq!(code, 0, "{out}{err}");
    let aspects_of = |argv: &[String]| -> Vec<String> {
        argv.iter()
            .find_map(|arg| arg.strip_prefix("--aspects="))
            .expect("aspects")
            .split(',')
            .map(|passed| dx_bep::aspect_key(passed).to_owned())
            .collect()
    };
    let commands = [Command::Format, Command::Lint, Command::Typecheck];
    let probes = harness.probe_argv.borrow();
    assert_eq!(probes.len(), 1, "{probes:?}");
    let probed = aspects_of(&probes[0]);
    for command in commands {
        for probe in dx_plan::plan::command_probes(&[command], None) {
            assert!(
                probed.iter().any(|passed| passed == probe),
                "{probe} missing from {probed:?}"
            );
        }
    }
    let builds = harness.build_argv.borrow();
    assert_eq!(builds.len(), 2, "{builds:?}");
    let passed = aspects_of(&builds[1]);
    let shards = std::collections::BTreeSet::from(["real_rust_lint".to_owned()]);
    for command in commands {
        let all = dx_plan::plan::spec(command).aspects.to_vec();
        let kept = dx_plan::plan::quality::select_aspects(all.clone(), Some(&shards));
        assert!(
            kept.len() < all.len(),
            "the probe drops shards of {command:?}"
        );
        for aspect in all {
            assert_eq!(
                passed.iter().any(|name| name == aspect),
                kept.contains(&aspect),
                "{aspect} in {passed:?}"
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
