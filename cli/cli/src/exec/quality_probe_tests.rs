use super::super::test_support::*;

fn aspects_of(argv: &[String]) -> Vec<String> {
    argv.iter()
        .find_map(|arg| arg.strip_prefix("--aspects="))
        .expect("aspects flag")
        .split(',')
        .map(|aspect| {
            aspect
                .rsplit_once('%')
                .map_or(aspect, |(_, name)| name)
                .to_owned()
        })
        .collect()
}

fn clean_harness(name: &str) -> Harness {
    let mut harness = Harness::new(name);
    harness.write_source("src/a.py", "x = 1\n");
    harness.results.insert(
        "//test:corpus".to_owned(),
        harness.valid_result(vec![], vec![]),
    );
    harness
}

#[test]
fn probed_shards_narrow_the_build_aspects() {
    let mut harness = clean_harness("probe-narrows");
    harness.probe_shards = Some(vec!["real_rust_lint".to_owned()]);
    let (code, _, _) = harness.run(&["lint", "--check", "--output=text"]);
    assert_eq!(code, 0);
    assert_eq!(harness.probe_argv.borrow().len(), 1);
    let builds = harness.build_argv.borrow();
    assert_eq!(builds.len(), 1);
    assert_eq!(
        aspects_of(&builds[0]),
        vec!["real_lint_aspect", "real_rust_lint_aspect"]
    );
    let probe = &harness.probe_argv.borrow()[0];
    assert_eq!(aspects_of(probe), vec!["real_lint_probe_aspect"]);
    let bep = probe
        .iter()
        .find_map(|arg| arg.strip_prefix("--build_event_json_file="))
        .expect("probe event file");
    assert!(
        !std::path::Path::new(bep).exists(),
        "the probe event file is removed"
    );
}

#[test]
fn failed_probe_falls_back_to_every_shard() {
    for (command, parsed) in [
        ("lint", crate::args::Command::Lint),
        ("format", crate::args::Command::Format),
        ("typecheck", crate::args::Command::Typecheck),
    ] {
        let harness = clean_harness(&format!("probe-fallback-{command}"));
        let (code, _, _) = harness.run(&[command, "--check", "--output=text"]);
        assert_eq!(code, 0, "{command}");
        assert_eq!(harness.probe_argv.borrow().len(), 1, "{command}");
        let builds = harness.build_argv.borrow();
        let all = crate::plan::spec(parsed).aspects.len();
        assert_eq!(aspects_of(&builds[0]).len(), all, "{command}");
    }
}

#[test]
fn probe_keeps_json_stdout_to_the_build_events() {
    let mut probed = clean_harness("probe-json");
    probed.probe_shards = Some(vec![]);
    let (probed_code, probed_out, _) = probed.run(&["lint", "--check", "--output=json"]);
    let plain = clean_harness("probe-json-fallback");
    let (plain_code, plain_out, _) = plain.run(&["lint", "--check", "--output=json"]);
    assert_eq!(probed_code, plain_code);
    assert_eq!(
        probed_out.lines().count(),
        plain_out.lines().count(),
        "the probe adds no JSON events"
    );
}

#[test]
fn interrupted_probe_cancels_the_phase_without_a_build() {
    let mut harness = clean_harness("probe-interrupted");
    harness.probe_code = Some(super::super::common::BAZEL_INTERRUPTED);
    let parsed = crate::args::parse(&dx_testing::cli::strings(&[
        "lint",
        "--check",
        "--output=text",
    ]))
    .expect("parse");
    let inv = crate::args::apply_here(&parsed, &harness.workspace, &harness.cwd).expect("here");
    let request = inv.quality_request().expect("quality request");
    let runner = harness.runner();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let run = super::execute_quality_phase(&request, harness.env(&runner, &mut out, &mut err));
    assert_eq!(run.kind, super::PhaseKind::Cancelled);
    assert_ne!(run.code, 0);
    assert_eq!(harness.probe_argv.borrow().len(), 1);
    assert!(
        harness.build_argv.borrow().is_empty(),
        "no build after an interrupted probe"
    );
}
