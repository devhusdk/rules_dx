use super::super::test_support::*;
use dx_fingerprint::baseline::{normalize_message, render_baseline, BaselineEntry, BaselineFile};
use quality_result::proto::{self, Convergence};
use quality_result::{decode_validated, encode_validated};

const SOURCE: &[u8] = b"x = 1\n";
const UNSTABLE: [(Convergence, u32); 2] = [
    (Convergence::Oscillation, 3),
    (Convergence::IterationLimit, 10),
];

fn settle(bytes: &[u8], convergence: Convergence, rounds: u32) -> Vec<u8> {
    let mut result = decode_validated(bytes).expect("valid result");
    result.convergence = convergence as i32;
    result.completed_rounds = rounds;
    encode_validated(&result).expect("encode")
}

fn harness_with(
    name: &str,
    diagnostics: Vec<proto::Diagnostic>,
    convergence: Convergence,
    rounds: u32,
) -> Harness {
    let mut harness = Harness::new(name);
    harness.write_source("src/a.py", "x = 1\n");
    let bytes = harness.valid_result(diagnostics, vec![]);
    harness.results.insert(
        "//test:corpus".to_owned(),
        settle(&bytes, convergence, rounds),
    );
    harness
}

fn cause(convergence: Convergence, rounds: u32) -> String {
    format!(
        "//test:corpus did not converge: {} after {rounds} round(s) of lint-tool",
        convergence.as_str_name()
    )
}

fn source(harness: &Harness) -> Vec<u8> {
    std::fs::read(harness.workspace.join("src/a.py")).expect("source")
}

#[test]
fn diagnostic_free_unstable_results_fail_every_quality_command() {
    for (convergence, rounds) in UNSTABLE {
        for words in [
            ["lint", "--output=text"],
            ["format", "--output=text"],
            ["typecheck", "--output=text"],
            ["lint", "--apply"],
            ["format", "--apply"],
        ] {
            let harness = harness_with("unstable-text", vec![], convergence, rounds);
            let (code, out, err) = harness.run(&words);
            assert_eq!(code, 1, "{words:?} {convergence:?}: {out}{err}");
            assert!(
                err.contains(&format!(
                    "dx: not_converged: {}",
                    cause(convergence, rounds)
                )),
                "{words:?}: {err}"
            );
            assert!(!err.contains("incomplete_results"), "{words:?}: {err}");
            assert!(!out.contains("Applied"), "{words:?}: {out}");
            assert_eq!(source(&harness), SOURCE, "{words:?} keeps the source bytes");
        }
    }
}

#[test]
fn stable_control_succeeds() {
    let harness = harness_with("stable-control", vec![], Convergence::Stable, 1);
    for words in [["lint", "--output=text"], ["format", "--apply"]] {
        let (code, out, err) = harness.run(&words);
        assert_eq!(code, 0, "{words:?}: {out}{err}");
        assert!(!err.contains("not_converged"), "{err}");
    }
}

#[test]
fn json_names_the_unstable_producer_and_keeps_the_result_set_complete() {
    for (convergence, rounds) in UNSTABLE {
        let harness = harness_with("unstable-json", vec![], convergence, rounds);
        let (code, out, _) = harness.run(&["lint", "--output=json"]);
        assert_eq!(code, 1, "{out}");
        let events = json_events(&out);
        let errors: Vec<&serde_json::Value> = events
            .iter()
            .filter(|event| event["event"] == "error")
            .collect();
        assert_eq!(errors.len(), 1, "{out}");
        assert_eq!(errors[0]["code"], "not_converged");
        assert_eq!(errors[0]["message"], cause(convergence, rounds).as_str());
        let finished = events.last().expect("finished");
        assert_eq!(finished["event"], "command_finished");
        assert_eq!(finished["exit_code"], 1);
        assert_eq!(finished["results_complete"], true);
    }
}

#[test]
fn severity_and_suppression_cannot_hide_nonconvergence() {
    let info = Harness::diagnostic_with(
        proto::Severity::Info as i32,
        "lint-tool",
        "src/a.py",
        "minor",
        false,
    );
    let below = harness_with("unstable-below", vec![info], Convergence::Oscillation, 2);
    let (code, out, err) = below.run(&["lint", "--fail-on=error", "--output=text"]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(out.contains("minor"), "the finding stays visible: {out}");
    assert!(err.contains("dx: not_converged:"), "{err}");

    let baselined = harness_with(
        "unstable-baselined",
        vec![Harness::diagnostic("unused", false)],
        Convergence::Oscillation,
        2,
    );
    let rel = "quality/baseline.json";
    baselined.write_source("dx.toml", &format!("[quality]\nbaseline = \"{rel}\"\n"));
    let recorded = render_baseline(&BaselineFile {
        entries: vec![BaselineEntry {
            tool: "lint-tool".to_owned(),
            rule: "lint-tool/rule".to_owned(),
            path: "src/a.py".to_owned(),
            target: String::new(),
            message: normalize_message("unused"),
            context: dx_digest::blake3(b"x = 1"),
            count: 1,
        }],
    })
    .expect("render");
    baselined.write_source(rel, &recorded);
    for words in [["lint", "--output=text"], ["lint", "--apply"]] {
        let (code, out, err) = baselined.run(&words);
        assert_eq!(code, 1, "{words:?}: {out}{err}");
        assert!(
            out.contains("unused [lint-tool/rule] (baselined)"),
            "{words:?}: {out}"
        );
        assert!(err.contains("dx: not_converged:"), "{words:?}: {err}");
        assert!(!out.contains("refreshed"), "{words:?}: {out}");
        assert_eq!(
            std::fs::read_to_string(baselined.workspace.join(rel)).expect("baseline"),
            recorded,
            "{words:?} never refreshes the baseline"
        );
    }
}

#[test]
fn apply_never_records_a_missing_baseline_from_an_unstable_result() {
    let harness = harness_with(
        "unstable-no-baseline",
        vec![],
        Convergence::IterationLimit,
        10,
    );
    harness.write_source(
        "dx.toml",
        "[quality]\nbaseline = \"quality/baseline.json\"\n",
    );
    let (code, _, err) = harness.run(&["lint", "--apply"]);
    assert_eq!(code, 1, "{err}");
    assert!(
        err.contains("baseline quality/baseline.json is missing and was not recorded"),
        "{err}"
    );
    assert!(!harness.workspace.join("quality/baseline.json").exists());
}

#[test]
fn independent_stable_fixes_still_apply_beside_an_unstable_producer() {
    let mut harness = Harness::new("unstable-partial");
    harness.write_source("src/a.py", "x = 1\n");
    harness.results.insert(
        "//a:fix".to_owned(),
        harness.valid_result(vec![], vec![harness.replacement(b"y")]),
    );
    let unstable = harness.result_full(vec![], vec![], vec![], vec![]);
    harness.results.insert(
        "//b:loop".to_owned(),
        settle(&unstable, Convergence::Oscillation, 4),
    );
    let (code, out, err) = harness.run(&["format", "--apply"]);
    assert_eq!(code, 1, "{out}{err}");
    assert_eq!(source(&harness), b"y = 1\n");
    assert!(out.contains("Applied 1 file(s)."), "{out}");
    assert!(
        err.contains(
            "dx: not_converged: //b:loop (//test:corpus) did not converge: \
             OSCILLATION after 4 round(s) of lint-tool"
        ),
        "{err}"
    );
}

#[test]
fn sarif_marks_the_unstable_tool_unsuccessful() {
    let harness = harness_with("unstable-sarif", vec![], Convergence::Oscillation, 3);
    let (code, _, err) = harness.run(&["lint", "--output=text", "--report=sarif=out.sarif"]);
    assert_eq!(code, 1, "{err}");
    let document: serde_json::Value =
        serde_json::from_slice(&std::fs::read(harness.workspace.join("out.sarif")).expect("sarif"))
            .expect("json");
    let run = &document["runs"][0];
    assert_eq!(run["tool"]["driver"]["name"], "lint-tool");
    assert_eq!(run["results"], serde_json::json!([]));
    assert_eq!(
        run["invocations"],
        serde_json::json!([{
            "executionSuccessful": false,
            "toolExecutionNotifications": [{
                "level": "error",
                "message": {"text": cause(Convergence::Oscillation, 3)},
            }],
        }])
    );
}

#[test]
fn unstable_apply_sarif_keeps_the_finding_at_its_checked_position() {
    let mut harness = Harness::new("unstable-sarif-shift");
    harness.write_source("src/a.py", "import os\nx = missing\n");
    let original = std::fs::read(harness.workspace.join("src/a.py")).expect("source");
    let mut before = Harness::diagnostic("undefined", false);
    before.start_byte = Some(14);
    before.end_byte = Some(21);
    let mut after = before.clone();
    after.start_byte = Some(4);
    after.end_byte = Some(11);
    let bytes = harness.result_full(
        vec![before],
        vec![after],
        vec![],
        vec![proto::FileSnapshot {
            path: "src/a.py".to_owned(),
            digest: dx_digest::blake3(&original).to_vec(),
        }],
    );
    let mut result = decode_validated(&bytes).expect("valid result");
    result.convergence = Convergence::Oscillation as i32;
    result.completed_rounds = 3;
    result.terminal_snapshot[0].digest = dx_digest::blake3(b"x = missing\n").to_vec();
    harness.results.insert(
        "//test:corpus".to_owned(),
        encode_validated(&result).expect("encode"),
    );
    let (code, out, err) = harness.run(&["lint", "--apply", "--report=sarif=out.sarif"]);
    assert_eq!(code, 1, "{out}{err}");
    assert_eq!(source(&harness), original);
    let document: serde_json::Value =
        serde_json::from_slice(&std::fs::read(harness.workspace.join("out.sarif")).expect("sarif"))
            .expect("json");
    let results = document["runs"][0]["results"].as_array().expect("results");
    assert_eq!(results.len(), 1, "{document}");
    let region = &results[0]["locations"][0]["physicalLocation"]["region"];
    assert_eq!(region["startLine"], 2, "{document}");
    assert_eq!(region["startColumn"], 5, "{document}");
}
