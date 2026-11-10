#[test]
fn log_tail_keeps_last_forty_lines() {
    let text: String = (1..=50).map(|n| format!("line {n}\n")).collect();
    let want: Vec<String> = (11..=50).map(|n| format!("line {n}")).collect();
    assert_eq!(super::log_tail(text.as_bytes()), Some(want.join("\n")));
}

#[test]
fn log_tail_returns_short_logs_whole() {
    assert_eq!(super::log_tail(b"a\nb\nc\n"), Some("a\nb\nc".to_owned()));
}

#[test]
fn log_tail_drops_partial_first_line_when_capped() {
    let lines: Vec<String> = (0..40)
        .map(|n| format!("{n:02}{}", "x".repeat(298)))
        .collect();
    let tail = super::log_tail(lines.join("\n").as_bytes()).unwrap();
    assert!(tail.len() <= super::LOG_TAIL_BYTES);
    let got: Vec<&str> = tail.split('\n').collect();
    assert!(got.iter().all(|l| l.len() == 300));
    assert_eq!(
        got,
        lines[40 - got.len()..]
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
    );
    assert!(!tail.starts_with('\n'));
}

#[test]
fn log_tail_cuts_one_oversize_multibyte_line_at_char_boundary() {
    let tail = super::log_tail("日".repeat(3500).as_bytes()).unwrap();
    assert!(!tail.is_empty() && tail.len() <= super::LOG_TAIL_BYTES);
}

#[test]
fn log_tail_is_none_for_blank_input() {
    assert_eq!(super::log_tail(b""), None);
    assert_eq!(super::log_tail(b"  \n\t\n\n"), None);
}

#[test]
fn log_tail_tolerates_invalid_utf8() {
    assert!(super::log_tail(b"ok\n\xff\xfe bad\n").is_some());
}

use super::super::test_support::*;
use super::stable_exec_path;
use std::path::Path;

#[test]
fn coverage_sources_returns_the_write_code_when_stdout_fails() {
    let account = dx_lcov::CellAccount::default();
    let got = super::coverage_sources(
        &super::Sources::default(),
        &account,
        dx_output::OutputMode::Json,
        false,
        &mut BrokenPipe,
        &mut Vec::new(),
    );
    assert_eq!(got, Err(dx_process::broken_pipe_code()));
}

#[test]
fn an_exec_path_reads_the_same_on_every_host() {
    let execroot = "/home/someone/.cache/bazel/_bazel_x/abc123/execroot/_main";
    assert_eq!(
        stable_exec_path(Path::new(&format!(
            "{execroot}/bazel-out/k8-fastbuild/bin/env/shard/doctor_test/test.xml"
        ))),
        "env/shard/doctor_test/test.xml"
    );
    assert_eq!(
        stable_exec_path(Path::new(&format!(
            "{execroot}/bazel-out/x64_windows-fastbuild/bin/env/shard/doctor_test/test.xml"
        ))),
        "env/shard/doctor_test/test.xml",
        "the configuration does not reach the report"
    );
    assert_eq!(
        stable_exec_path(Path::new(&format!("{execroot}/bin/env/shard/test.xml"))),
        "env/shard/test.xml"
    );
}

#[test]
fn a_windows_exec_path_keeps_its_forward_slashes() {
    assert_eq!(
        stable_exec_path(Path::new(
            r"C:/bz/out/execroot/_main/bazel-out/x64_windows-fastbuild/bin/env/shard/test.xml"
        )),
        "env/shard/test.xml"
    );
}

#[test]
fn a_path_outside_the_build_directory_keeps_its_file_name() {
    assert_eq!(
        stable_exec_path(Path::new("/tmp/somewhere/test.xml")),
        "test.xml"
    );
    assert_eq!(stable_exec_path(Path::new("test.xml")), "test.xml");
}

/// A local `file://` URI for a path that is not there, spelled the way this host spells one.
///
/// Windows only reads a local `file://` URI back as a path when it carries a drive letter, so
/// the rooted spelling every other host accepts would read as a remote host there.
fn missing_uri(name: &str) -> String {
    if cfg!(windows) {
        format!("file:///C:/nonexistent/{name}")
    } else {
        format!("file:///nonexistent/{name}")
    }
}

const MINIMAL_TEST_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?><testsuites><testsuite name="s"><testcase name="passes" classname="c" time="0.1"/></testsuite></testsuites>"#;

const MINIMAL_LCOV: &str = "SF:src/a.py\nDA:1,1\nend_of_record\n";

#[test]
fn test_junit_file_report_succeeds() {
    let harness = Harness::new("test-junit");
    let uri = write_bep_artifact(&harness, "test.xml", MINIMAL_TEST_XML.as_bytes());
    let harness = Harness {
        raw_bep: Some(vec![test_result_line(
            "//a:t",
            &[(String::from("test.xml"), uri)],
        )]),
        ..harness
    };
    let (code, out, _) = harness.run(&["test", "--output=text", "--report=junit=out.xml"]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("Wrote junit report to out.xml."));
    let document = std::fs::read(harness.workspace.join("out.xml")).expect("junit");
    let text = String::from_utf8(document).expect("utf8");
    assert!(text.contains("<testsuites name=\"dx\""), "{text}");
    assert!(text.contains("<testsuite name=\"//a:t\""), "{text}");
}

#[test]
fn test_stdout_report_owns_stdout() {
    let harness = Harness::new("test-stdout");
    let uri = write_bep_artifact(&harness, "test-stdout.xml", MINIMAL_TEST_XML.as_bytes());
    let harness = Harness {
        raw_bep: Some(vec![test_result_line(
            "//a:t",
            &[(String::from("test.xml"), uri)],
        )]),
        ..harness
    };
    let (code, out, _) = harness.run(&["test", "--output=text", "--report=junit=-"]);
    assert_eq!(code, 0);
    assert!(out.contains("<testsuites name=\"dx\""), "{out}");
    assert!(!out.contains("Running test"), "{out}");
}

#[test]
fn test_missing_artifacts_are_incomplete() {
    let harness = Harness {
        raw_bep: Some(vec![String::from(
            "{\"id\": {\"testResult\": {\"label\": \"//a:t\"}}, \"testResult\": {\"status\": \"PASSED\"}}",
        )]),
        ..Harness::new("test-empty")
    };
    let (code, _, err) = harness.run(&["test", "--output=text"]);
    assert_eq!(code, 1);
    assert!(err.contains("incomplete_results"), "{err}");
}

#[test]
fn test_preserves_bazel_failure_code() {
    let harness = Harness::new("test-bazel-fails");
    let uri = write_bep_artifact(&harness, "fail.xml", MINIMAL_TEST_XML.as_bytes());
    let harness = Harness {
        bazel_code: 4,
        raw_bep: Some(vec![test_result_line(
            "//a:t",
            &[(String::from("test.xml"), uri)],
        )]),
        ..harness
    };
    let (code, _, _) = harness.run(&["test", "--output=text"]);
    assert_eq!(code, 4);
}

#[test]
fn coverage_lcov_file_report_succeeds() {
    let harness = Harness::new("cov-ok");
    let uri = write_bep_artifact(&harness, "coverage.dat", MINIMAL_LCOV.as_bytes());
    let harness = Harness {
        raw_bep: Some(vec![test_result_line(
            "//a:t",
            &[(String::from("test.lcov"), uri)],
        )]),
        ..harness
    };
    let (code, out, _) = harness.run(&["coverage", "--output=text", "--report=lcov=out.lcov"]);
    assert_eq!(code, 0, "{out}");
    let document = std::fs::read(harness.workspace.join("out.lcov")).expect("lcov");
    assert_eq!(document, MINIMAL_LCOV.as_bytes());
}

#[test]
fn coverage_invalid_tracefile_fails() {
    let harness = Harness::new("cov-bad");
    let uri = write_bep_artifact(&harness, "bad.dat", b"not lcov");
    let harness = Harness {
        raw_bep: Some(vec![test_result_line(
            "//a:t",
            &[(String::from("test.lcov"), uri)],
        )]),
        ..harness
    };
    let (code, _, err) = harness.run(&["coverage", "--output=text"]);
    assert_eq!(code, 1);
    assert!(err.contains("incomplete_results"), "{err}");
}

#[test]
fn coverage_empty_tracefile_skipped_when_valid_present() {
    let harness = Harness::new("cov-empty-skipped");
    let empty_uri = write_bep_artifact(&harness, "empty.dat", b"");
    let valid_uri = write_bep_artifact(&harness, "valid.dat", MINIMAL_LCOV.as_bytes());
    let harness = Harness {
        raw_bep: Some(vec![
            test_result_line("//a:empty", &[(String::from("test.lcov"), empty_uri)]),
            test_result_line("//a:valid", &[(String::from("test.lcov"), valid_uri)]),
        ]),
        ..harness
    };
    let harness = owning_a(harness);
    let (code, _, err) = harness.run(&["coverage", "--output=text", "--min-coverage=100"]);
    assert_eq!(code, 0, "{err}");
    assert!(err.contains("meets minimum 100%"), "{err}");
}

#[test]
fn coverage_whitespace_only_tracefile_skipped() {
    let harness = Harness::new("cov-ws-skipped");
    let ws_uri = write_bep_artifact(&harness, "ws.dat", b"  \n\t\n");
    let valid_uri = write_bep_artifact(&harness, "valid.dat", MINIMAL_LCOV.as_bytes());
    let harness = Harness {
        raw_bep: Some(vec![
            test_result_line("//a:ws", &[(String::from("test.lcov"), ws_uri)]),
            test_result_line("//a:valid", &[(String::from("test.lcov"), valid_uri)]),
        ]),
        ..harness
    };
    let harness = owning_a(harness);
    let (code, _, err) = harness.run(&["coverage", "--output=text", "--min-coverage=100"]);
    assert_eq!(code, 0, "{err}");
    assert!(err.contains("meets minimum 100%"), "{err}");
}

const HALF_LCOV: &str = "SF:src/a.py\nDA:1,1\nDA:2,0\nend_of_record\n";

#[test]
fn coverage_min_coverage_passes_at_threshold() {
    let harness = coverage_harness("cov-threshold-ok", MINIMAL_LCOV.as_bytes());
    let harness = owning_a(harness);
    let (code, _, err) = harness.run(&["coverage", "--output=text", "--min-coverage=100"]);
    assert_eq!(code, 0, "{err}");
    assert!(err.contains("meets minimum 100%"), "{err}");
}

#[test]
fn coverage_merges_the_lcov_a_php_test_writes() {
    let lcov = "SF:php/tests/fixtures/hello/src/Hello/Greeter.php\nDA:13,1\nDA:17,1\nLF:2\nLH:2\nend_of_record\n";
    let harness = coverage_harness("cov-php-pcov", lcov.as_bytes());
    let harness = with_sources(
        harness,
        "v1 cell linux_x86_64\nprod source php/tests/fixtures/hello/src/Hello/Greeter.php\n",
    );
    let (code, _, err) = harness.run(&["coverage", "--output=text", "--min-coverage=100"]);
    assert_eq!(code, 0, "{err}");
    assert!(err.contains("(2/2 lines) meets minimum 100%"), "{err}");
}

#[test]
fn coverage_merges_the_lcov_a_julia_test_writes() {
    let lcov =
        "SF:julia/tests/fixtures/hello/src/Greet.jl\nDA:1,1\nDA:2,1\nLF:2\nLH:2\nend_of_record\n";
    let harness = coverage_harness("cov-julia", lcov.as_bytes());
    let harness = with_sources(
        harness,
        "v1 cell linux_x86_64\nprod source julia/tests/fixtures/hello/src/Greet.jl\n",
    );
    let (code, _, err) = harness.run(&["coverage", "--output=text", "--min-coverage=100"]);
    assert_eq!(code, 0, "{err}");
    assert!(err.contains("(2/2 lines) meets minimum 100%"), "{err}");
}

#[test]
fn coverage_merges_the_lcov_a_haskell_test_writes() {
    let lcov = "SF:haskell/private/coverage/fixture/Greeter.hs\nDA:4,1\nDA:7,0\nLF:2\nLH:1\nend_of_record\n";
    let harness = coverage_harness("cov-haskell-hpc", lcov.as_bytes());
    let harness = with_sources(
        harness,
        "v1 cell linux_x86_64\nprod source haskell/private/coverage/fixture/Greeter.hs\n",
    );
    let (code, _, err) = harness.run(&["coverage", "--output=text", "--min-coverage=50"]);
    assert_eq!(code, 0, "{err}");
    assert!(
        err.contains("50.00% (1/2 lines) meets minimum 50%"),
        "{err}"
    );
}

#[test]
fn coverage_merges_the_lcov_an_ocaml_test_writes() {
    let lcov = "SF:ocaml/tests/runtime/greet.ml\nDA:1,1\nDA:3,0\nLF:2\nLH:1\nend_of_record\n";
    let harness = coverage_harness("cov-ocaml-bisect", lcov.as_bytes());
    let harness = with_sources(
        harness,
        "v1 cell linux_x86_64\nprod source ocaml/tests/runtime/greet.ml\n",
    );
    let (code, _, err) = harness.run(&["coverage", "--output=text", "--min-coverage=50"]);
    assert_eq!(code, 0, "{err}");
    assert!(
        err.contains("50.00% (1/2 lines) meets minimum 50%"),
        "{err}"
    );
}

#[test]
fn coverage_min_coverage_fails_below_threshold() {
    let harness = coverage_harness("cov-threshold-low", HALF_LCOV.as_bytes());
    let harness = with_sources(harness, PY_SOURCES);
    let (code, _, err) = harness.run(&["coverage", "--output=text", "--min-coverage=80"]);
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("coverage_below_minimum"), "{err}");
    assert!(err.contains("50.00% (1/2 lines)"), "{err}");
}

#[test]
fn coverage_min_coverage_failure_reports_json_event() {
    let harness = coverage_harness("cov-threshold-json", HALF_LCOV.as_bytes());
    let harness = with_sources(harness, PY_SOURCES);
    let (code, out, _) = harness.run(&["coverage", "--output=json", "--min-coverage=80"]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("coverage_below_minimum"), "{out}");
}

#[test]
fn coverage_min_coverage_fails_without_executable_lines() {
    let harness = coverage_harness("cov-threshold-empty", b"SF:src/a.py\nend_of_record\n");
    let harness = with_sources(harness, PY_SOURCES);
    let (code, _, err) = harness.run(&["coverage", "--output=text", "--min-coverage=80"]);
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("no executable lines"), "{err}");
}

#[test]
fn coverage_min_coverage_rejects_invalid_markers() {
    let harness = Harness::new("cov-threshold-markers");
    let source = harness.workspace.join("src/lib.rs");
    std::fs::create_dir_all(source.parent().expect("parent")).expect("mkdir");
    std::fs::write(&source, "// LCOV_EXCL_LINE\nfn a() {}\n").expect("write");
    let uri = write_bep_artifact(
        &harness,
        "coverage.dat",
        b"SF:src/lib.rs\nDA:1,1\nDA:2,1\nend_of_record\n",
    );
    let harness = Harness {
        raw_bep: Some(vec![test_result_line(
            "//a:t",
            &[(String::from("test.lcov"), uri)],
        )]),
        ..harness
    };
    let harness = with_sources(harness, "v1 cell linux_x86_64\nprod source src/lib.rs\n");
    let (code, _, err) = harness.run(&["coverage", "--output=text", "--min-coverage=80"]);
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("coverage_below_minimum"), "{err}");
}

#[test]
fn coverage_min_coverage_counts_d_sources() {
    let harness = Harness::new("cov-threshold-d");
    let source = harness.workspace.join("src/adder.d");
    std::fs::create_dir_all(source.parent().expect("parent")).expect("mkdir");
    std::fs::write(&source, "int add(int a, int b) {\n    return a + b;\n}\n").expect("write");
    let uri = write_bep_artifact(
        &harness,
        "coverage.dat",
        b"SF:src/adder.d\nDA:1,1\nDA:2,0\nend_of_record\n",
    );
    let harness = Harness {
        raw_bep: Some(vec![test_result_line(
            "//a:t",
            &[(String::from("test.lcov"), uri)],
        )]),
        ..harness
    };
    let harness = with_sources(harness, "v1 cell linux_x86_64\nprod source src/adder.d\n");
    let (code, _, err) = harness.run(&["coverage", "--output=text", "--min-coverage=80"]);
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("coverage_below_minimum"), "{err}");
    assert!(err.contains("1/2 lines"), "{err}");
}

const SOURCES_ASPECT: &str = "//core/quality:coverage_sources.bzl%dx_coverage_sources_aspect";

const PY_SOURCES: &str = "v1 cell linux_x86_64\nprod source src/a.py\n";

/// Adds one `//a:lib` source manifest to a harness's build events.
fn with_sources(harness: Harness, manifest: &str) -> Harness {
    write_manifest_sources(&harness, manifest);
    let uri = write_bep_artifact(&harness, "sources.txt", manifest.as_bytes());
    let mut lines = vec![
        sources_set("s1", &uri),
        sources_completed("//a:lib", true, "s1"),
    ];
    lines.extend(harness.raw_bep.clone().unwrap_or_default());
    Harness {
        raw_bep: Some(lines),
        ..harness
    }
}

fn sources_completed(label: &str, success: bool, set: &str) -> String {
    let mut completed = serde_json::json!({"success": success});
    if success {
        completed["outputGroup"] =
            serde_json::json!([{"name": "dx_coverage_sources", "fileSets": [{"id": set}]}]);
    }
    serde_json::json!({
        "id": {"targetCompleted": {"label": label, "aspect": SOURCES_ASPECT}},
        "completed": completed,
    })
    .to_string()
}

fn sources_set(set: &str, uri: &str) -> String {
    serde_json::json!({
        "id": {"namedSet": {"id": set}},
        "namedSetOfFiles": {"files": [{"name": "lib.dx_coverage_sources.txt", "uri": uri}]},
    })
    .to_string()
}

/// Prepends a manifest owning `src/a.py`, so `--min-coverage` can account the run's sources.
fn owning_a(mut harness: Harness) -> Harness {
    write_manifest_sources(&harness, "prod source src/a.py\n");
    let uri = write_bep_artifact(
        &harness,
        "sources.txt",
        b"v1 cell linux_x86_64\nprod source src/a.py\n",
    );
    let lines = [
        sources_set("s1", &uri),
        sources_completed("//a:lib", true, "s1"),
    ];
    if let Some(bep) = harness.raw_bep.as_mut() {
        bep.splice(0..0, lines);
    }
    harness
}

fn sources_harness(name: &str, manifest: &str, lcov: &str) -> Harness {
    let harness = Harness::new(name);
    write_manifest_sources(&harness, manifest);
    let lcov_uri = write_bep_artifact(&harness, "coverage.dat", lcov.as_bytes());
    let manifest_uri = write_bep_artifact(&harness, "sources.txt", manifest.as_bytes());
    Harness {
        raw_bep: Some(vec![
            sources_set("s1", &manifest_uri),
            sources_completed("//a:lib", true, "s1"),
            test_result_line("//a:t", &[(String::from("test.lcov"), lcov_uri)]),
        ]),
        ..harness
    }
}

#[test]
fn coverage_absent_production_source_is_a_notice() {
    let harness = sources_harness(
        "cov-sources-absent",
        "v1 cell linux_x86_64\nprod source src/a.py\nprod source src/b.py\n",
        MINIMAL_LCOV,
    );
    let (code, _, err) = harness.run(&["coverage", "--output=text"]);
    assert_eq!(code, 0, "{err}");
    assert!(
            err.contains(
                "dx: coverage_sources_incomplete: production source absent from coverage report: src/b.py"
            ),
            "{err}"
        );
}

#[test]
fn coverage_sources_report_lists_owned_and_absent() {
    let harness = sources_harness(
        "cov-sources-report",
        "v1 cell linux_x86_64\nprod source src/a.py\nprod source src/b.py\ntest source src/a_test.py\n",
        MINIMAL_LCOV,
    );
    let path = harness.workspace.join("sources.json");
    let (code, _, err) = harness.run(&[
        "coverage",
        "--output=text",
        &format!("--report=sources={}", path.display()),
    ]);
    assert_eq!(code, 0, "{err}");
    let document: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).expect("sources report")).unwrap();
    assert_eq!(document["record_version"], serde_json::json!(2));
    assert_eq!(document["orphans"], serde_json::json!([]));
    assert_eq!(document["absent"], serde_json::json!(["src/b.py"]));
    assert_eq!(document["owned"]["src/a.py"], serde_json::json!("eligible"));
    assert_eq!(
        document["owned"]["src/a_test.py"],
        serde_json::json!("support")
    );
}

const ORPHAN_MANIFEST: &str = "v1 cell linux_x86_64\nprod source src/a.py\n";
const ORPHAN_MESSAGE: &str = "tracked source not owned by any Bazel target: src/orphan.py";

/// Scripts a clean Git checkout whose tracked files include a used and an unused source.
fn script_checkout(harness: &Harness) {
    std::fs::create_dir_all(harness.workspace.join("src")).expect("mkdir");
    std::fs::write(harness.workspace.join("src/BUILD.bazel"), "").expect("build");
    harness.query.script_checkout_git(0, "true\n");
    harness.query.script_checkout_git(0, "abc123\n");
    harness.query.script_checkout_git(0, "");
    harness.query.script_checkout_git(0, "true\n");
    harness
        .query
        .script_checkout_git(0, "src/a.py\0src/orphan.py\0src/BUILD.bazel\0");
    harness.query.script_graph(&[dx_testing::cli::graph_rule(
        "//src:lib",
        &["//src:a.py"],
        &[],
    )]);
}

#[test]
fn coverage_tracked_orphan_fails_min_coverage() {
    let harness = sources_harness("cov-orphan-min", ORPHAN_MANIFEST, MINIMAL_LCOV);
    script_checkout(&harness);
    let (code, _, err) = harness.run(&["coverage", "--output=text", "--min-coverage=100"]);
    assert_eq!(code, 1, "{err}");
    assert!(err.contains(ORPHAN_MESSAGE), "{err}");
}

#[test]
fn coverage_tracked_orphan_is_a_notice_without_minimum() {
    let harness = sources_harness("cov-orphan-notice", ORPHAN_MANIFEST, MINIMAL_LCOV);
    script_checkout(&harness);
    let path = harness.workspace.join("sources.json");
    let (code, _, err) = harness.run(&[
        "coverage",
        "--output=text",
        &format!("--report=sources={}", path.display()),
    ]);
    assert_eq!(code, 0, "{err}");
    assert!(err.contains(ORPHAN_MESSAGE), "{err}");
    let document: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).expect("sources report")).unwrap();
    assert_eq!(document["orphans"], serde_json::json!(["src/orphan.py"]));
    assert_eq!(
        document["candidate"],
        serde_json::json!({"sha": "abc123", "dirty": false})
    );
}

#[test]
fn coverage_tracked_check_skips_scoped_runs() {
    let harness = sources_harness("cov-orphan-scoped", ORPHAN_MANIFEST, MINIMAL_LCOV);
    script_checkout(&harness);
    let (code, _, err) = harness.run(&[
        "coverage",
        "--output=text",
        "--min-coverage=100",
        "//src:lib",
    ]);
    assert_eq!(code, 0, "{err}");
    assert!(!err.contains(ORPHAN_MESSAGE), "{err}");
}

#[test]
fn coverage_tracked_check_skips_non_git_workspace() {
    let harness = sources_harness("cov-orphan-nogit", ORPHAN_MANIFEST, MINIMAL_LCOV);
    harness.query.script_checkout_git(128, "");
    harness.query.script_checkout_git(128, "");
    let (code, _, err) = harness.run(&["coverage", "--output=text", "--min-coverage=100"]);
    assert_eq!(code, 0, "{err}");
    assert!(
        err.contains("dx: tracked sources unavailable: not a Git work tree"),
        "{err}"
    );
}

#[test]
fn coverage_tracked_query_failure_fails_min_coverage() {
    let harness = sources_harness("cov-orphan-query", ORPHAN_MANIFEST, MINIMAL_LCOV);
    script_checkout(&harness);
    harness.query.outputs.borrow_mut().clear();
    harness.query.script_failure(1, "ERROR: boom\n");
    let (code, _, err) = harness.run(&["coverage", "--output=text", "--min-coverage=100"]);
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("tracked sources unavailable:"), "{err}");
}

#[test]
fn coverage_event_carries_sha_dirty_and_collector() {
    let mut harness = sources_harness("cov-event-candidate", ORPHAN_MANIFEST, MINIMAL_LCOV);
    script_checkout(&harness);
    harness.raw_bep.as_mut().expect("bep").insert(
        0,
        r#"{"id":{"started":{}},"started":{"buildToolVersion":"9.0.0"}}"#.to_owned(),
    );
    let (code, out, err) = harness.run(&["coverage", "--output=json"]);
    assert_eq!(code, 0, "{err}");
    let event = out
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .find(|event| event["event"] == "coverage")
        .expect("coverage event");
    assert_eq!(event["sha"], "abc123");
    assert_eq!(event["dirty"], false);
    assert_eq!(event["collector"], "bazel 9.0.0");
}

#[test]
fn coverage_zero_hit_line_fails_min_coverage() {
    let harness = sources_harness("cov-sources-zero-hit", PY_SOURCES, HALF_LCOV);
    let (code, _, err) = harness.run(&["coverage", "--output=text", "--min-coverage=100"]);
    assert_eq!(code, 1, "{err}");
    assert!(
            err.contains(
                "dx: coverage 50.00% (1/2 lines) below minimum 100% in package linux_x86_64 (coverage_below_minimum)"
            ),
            "{err}"
        );
}

#[test]
fn coverage_report_without_manifests_fails_min_coverage() {
    let harness = coverage_harness("cov-sources-none", MINIMAL_LCOV.as_bytes());
    let (code, _, err) = harness.run(&["coverage", "--output=text", "--min-coverage=100"]);
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("coverage_below_minimum"), "{err}");
    assert!(
        err.contains("no coverage source manifests were reported"),
        "{err}"
    );
}

#[test]
fn coverage_event_reports_the_cell_account() {
    let harness = sources_harness(
        "cov-sources-event",
        "v1 cell linux_x86_64\nprod source src/a.py\nprod source src/b.py\n",
        MINIMAL_LCOV,
    );
    let (code, out, _) = harness.run(&["coverage", "--output=json", "--min-coverage=100"]);
    assert_eq!(code, 1, "{out}");
    let events: Vec<serde_json::Value> = out
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect();
    let coverage: Vec<&serde_json::Value> = events
        .iter()
        .filter(|event| event["event"] == "coverage")
        .collect();
    assert_eq!(coverage.len(), 1, "{out}");
    let event = coverage[0];
    assert_eq!(event["cell"], "linux_x86_64", "{event}");
    assert_eq!(event["covered_lines"], 1, "{event}");
    assert_eq!(event["eligible_lines"], 1, "{event}");
    assert_eq!(event["minimum"], 100, "{event}");
    assert_eq!(event["result"], "fail", "{event}");
    assert_eq!(event["errors"], 1, "{event}");
    assert_eq!(event["dx_version"], dx_adopt::DX_VERSION, "{event}");
    assert!(
        events
            .iter()
            .any(|event| event["code"] == "coverage_below_minimum"),
        "{out}"
    );
}

#[test]
fn coverage_event_passes_a_clean_cell_without_minimum() {
    let harness = sources_harness("cov-sources-event-clean", PY_SOURCES, MINIMAL_LCOV);
    let (code, out, _) = harness.run(&["coverage", "--output=json"]);
    assert_eq!(code, 0, "{out}");
    let event = out
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .find(|event| event["event"] == "coverage")
        .unwrap_or_else(|| panic!("no coverage event: {out}"));
    assert_eq!(event["result"], "pass", "{event}");
    assert!(event.get("minimum").is_none(), "{event}");
}

#[test]
fn coverage_sources_counts_line_and_silent_test_sources() {
    let harness = sources_harness(
        "cov-sources-counts",
        "v1 cell linux_x86_64\nprod source src/a.py\ntest source src/a_test.py\n",
        MINIMAL_LCOV,
    );
    let (code, _, err) = harness.run(&["coverage", "--output=text"]);
    assert_eq!(code, 0, "{err}");
    assert!(
            err.contains(
                "dx: coverage sources linux_x86_64: 1 raw, 0 excluded, 1 eligible, 1 covered lines; 1 eligible, 1 support, 0 excluded, 0 unsupported files"
            ),
            "{err}"
        );
    assert!(!err.contains("coverage_sources_incomplete"), "{err}");
}

#[test]
fn coverage_sources_counts_perl_sources() {
    let harness = sources_harness(
        "cov-sources-perl",
        "v1 cell linux_x86_64\nprod source src/a.pm\ntest source src/a_test.pl\n",
        "SF:src/a.pm\nDA:1,1\nend_of_record\n",
    );
    let (code, _, err) = harness.run(&["coverage", "--output=text"]);
    assert_eq!(code, 0, "{err}");
    assert!(
        err.contains(
            "1 eligible, 1 covered lines; 1 eligible, 1 support, 0 excluded, 0 unsupported files"
        ),
        "{err}"
    );
}

#[test]
fn coverage_sources_notice_is_a_json_event() {
    let harness = sources_harness(
        "cov-sources-json",
        "v1 cell linux_x86_64\nprod source src/a.py\nprod source src/b.py\n",
        MINIMAL_LCOV,
    );
    let (code, out, _) = harness.run(&["coverage", "--output=json"]);
    assert_eq!(code, 0, "{out}");
    let notice = out
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .find(|event| event["code"] == "coverage_sources_incomplete")
        .unwrap_or_else(|| panic!("no notice: {out}"));
    assert_eq!(notice["event"], "notice", "{notice}");
    assert_eq!(notice["severity"], "warning", "{notice}");
    assert_eq!(
        notice["message"],
        "production source absent from coverage report: src/b.py"
    );
    let counts = out
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .find(|event| event["code"] == "coverage_sources")
        .unwrap_or_else(|| panic!("no counts notice: {out}"));
    assert_eq!(counts["severity"], "info", "{counts}");
    assert!(
        counts["message"]
            .as_str()
            .is_some_and(|message| message.starts_with("coverage sources linux_x86_64: ")),
        "{counts}"
    );
}

#[test]
fn coverage_invalid_source_manifest_is_a_notice() {
    let harness = sources_harness("cov-sources-invalid", "v1 cell unknown\n", MINIMAL_LCOV);
    let (code, _, err) = harness.run(&["coverage", "--output=text"]);
    assert_eq!(code, 0, "{err}");
    assert!(
        err.contains("dx: coverage_sources_incomplete: invalid coverage source manifest"),
        "{err}"
    );
    assert!(err.contains("unknown cell"), "{err}");
}

#[test]
fn coverage_absent_production_source_fails_min_coverage() {
    let harness = sources_harness(
        "cov-sources-absent-gate",
        "v1 cell linux_x86_64\nprod source src/a.py\nprod source src/b.py\n",
        MINIMAL_LCOV,
    );
    let (code, _, err) = harness.run(&["coverage", "--output=text", "--min-coverage=100"]);
    assert_eq!(code, 1, "{err}");
    assert!(!err.contains("meets minimum"), "{err}");
    assert!(
            err.contains(
                "dx: coverage 100.00% (1/1 lines) in package linux_x86_64 has 1 coverage source gap (see"
            ),
            "{err}"
        );
    assert!(
            err.contains(
                "dx: coverage_sources_incomplete: production source absent from coverage report: src/b.py"
            ),
            "{err}"
        );
}

#[test]
fn coverage_unowned_report_path_fails_min_coverage() {
    let harness = sources_harness(
        "cov-sources-unowned-gate",
        "v1 cell linux_x86_64\nprod source src/other.py\n",
        "SF:src/a.py\nDA:1,1\nend_of_record\nSF:src/other.py\nDA:1,1\nend_of_record\n",
    );
    let (code, _, err) = harness.run(&["coverage", "--output=text", "--min-coverage=100"]);
    assert_eq!(code, 1, "{err}");
    assert!(
            err.contains(
                "dx: coverage_sources_incomplete: coverage report path not owned by any target: src/a.py"
            ),
            "{err}"
        );
}

#[test]
fn coverage_invalid_source_manifest_fails_min_coverage() {
    let harness = sources_harness(
        "cov-sources-invalid-gate",
        "v1 cell unknown\n",
        MINIMAL_LCOV,
    );
    let (code, _, err) = harness.run(&["coverage", "--output=text", "--min-coverage=100"]);
    assert_eq!(code, 1, "{err}");
    assert!(
        err.contains("dx: coverage_sources_incomplete: invalid coverage source manifest"),
        "{err}"
    );
}

#[test]
fn coverage_sources_error_is_a_json_error_under_min_coverage() {
    let harness = sources_harness(
        "cov-sources-json-gate",
        "v1 cell linux_x86_64\nprod source src/a.py\nprod source src/b.py\n",
        MINIMAL_LCOV,
    );
    let (code, out, _) = harness.run(&["coverage", "--output=json", "--min-coverage=100"]);
    assert_eq!(code, 1, "{out}");
    let error = out
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .find(|event| event["code"] == "coverage_sources_incomplete")
        .unwrap_or_else(|| panic!("no error: {out}"));
    assert_eq!(error["event"], "error", "{error}");
    assert_eq!(
        error["message"],
        "production source absent from coverage report: src/b.py"
    );
}

#[test]
fn coverage_test_only_sources_pass_min_coverage() {
    let harness = sources_harness(
        "cov-sources-test-only-gate",
        "v1 cell linux_x86_64\nprod source src/a.py\ntest source src/a_test.py\n",
        MINIMAL_LCOV,
    );
    let (code, _, err) = harness.run(&["coverage", "--output=text", "--min-coverage=100"]);
    assert_eq!(code, 0, "{err}");
    assert!(!err.contains("coverage_sources_incomplete"), "{err}");
}

#[test]
fn coverage_missing_source_manifest_blocks_min_coverage() {
    let harness = Harness::new("cov-sources-missing");
    let lcov_uri = write_bep_artifact(&harness, "coverage.dat", MINIMAL_LCOV.as_bytes());
    let harness = Harness {
        raw_bep: Some(vec![
            sources_completed("//a:lib", false, "s1"),
            test_result_line("//a:t", &[(String::from("test.lcov"), lcov_uri)]),
        ]),
        ..harness
    };
    let (code, _, err) = harness.run(&["coverage", "--output=text", "--min-coverage=100"]);
    assert_eq!(code, 1, "{err}");
    assert!(
        err.contains("dx: incomplete_results: //a:lib did not report its coverage source manifest"),
        "{err}"
    );
    assert!(!err.contains("meets minimum"), "{err}");
}

#[test]
fn coverage_unreadable_source_manifest_is_incomplete() {
    let harness = Harness::new("cov-sources-unreadable");
    let lcov_uri = write_bep_artifact(&harness, "coverage.dat", MINIMAL_LCOV.as_bytes());
    let harness = Harness {
        raw_bep: Some(vec![
            sources_set("s1", &missing_uri("sources.txt")),
            sources_completed("//a:lib", true, "s1"),
            test_result_line("//a:t", &[(String::from("test.lcov"), lcov_uri)]),
        ]),
        ..harness
    };
    let (code, _, err) = harness.run(&["coverage", "--output=text"]);
    assert_eq!(code, 1, "{err}");
    assert!(
        err.contains("coverage source manifests are unreadable"),
        "{err}"
    );
}

#[test]
fn test_unreadable_bep_is_operational() {
    let mut harness = Harness::new("test-nobep");
    harness.skip_bep = true;
    let (code, _, err) = harness.run(&["test", "--output=text"]);
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("unreadable_bep"), "{err}");
}

#[test]
fn test_invalid_bep_is_operational() {
    let harness = Harness {
        raw_bep: Some(vec![String::from("not json")]),
        ..Harness::new("test-badbep")
    };
    let (code, _, err) = harness.run(&["test", "--output=text"]);
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("invalid_bep"), "{err}");
}

#[test]
fn test_missing_bep_keeps_bazel_code() {
    let mut harness = Harness::new("test-nobep-failed");
    harness.skip_bep = true;
    harness.bazel_code = 2;
    let (code, out, _) = harness.run(&[
        "test",
        "--output=json",
        "--report=results=r.json",
        "--report=junit=j.xml",
    ]);
    assert_eq!(code, 2, "{out}");
    let events = json_events(&out);
    assert!(
        events_of_kind(&events, "notice")
            .iter()
            .any(|event| event["code"] == "build_events_incomplete"),
        "{out}"
    );
    let errors: Vec<_> = events_of_kind(&events, "error")
        .into_iter()
        .map(|event| event["code"].as_str().unwrap_or_default().to_owned())
        .collect();
    assert!(errors.contains(&"incomplete_results".to_owned()), "{out}");
    assert!(errors.contains(&"bazel_failed".to_owned()), "{out}");
    assert!(!out.contains("unreadable_bep"), "{out}");
    assert_eq!(
        event(&events, "command_finished")["exit_code"],
        serde_json::json!(2)
    );
    let document: serde_json::Value = serde_json::from_slice(
        &std::fs::read(harness.workspace.join("r.json")).expect("results report"),
    )
    .expect("results json");
    assert_eq!(document["exit_code"], serde_json::json!(2));
    assert_eq!(document["complete"], serde_json::json!(false));
    assert_eq!(document["results"], serde_json::json!([]));
    let junit = std::fs::read_to_string(harness.workspace.join("j.xml")).expect("junit");
    assert!(junit.contains("dx.infrastructure"), "{junit}");
    assert!(junit.contains("build events are unavailable"), "{junit}");
}

#[test]
fn test_cut_off_bep_keeps_bazel_code_and_results() {
    let harness = Harness::new("test-cutbep-failed");
    let uri = write_bep_artifact(&harness, "ok.xml", MINIMAL_TEST_XML.as_bytes());
    let harness = Harness {
        bazel_code: 37,
        unterminated_bep: true,
        raw_bep: Some(vec![
            test_result_line("//a:t", &[(String::from("test.xml"), uri)]),
            String::from(r#"{"id":{"testRes"#),
        ]),
        ..harness
    };
    let (code, out, _) = harness.run(&[
        "test",
        "--output=json",
        "--report=results=r.json",
        "--report=junit=j.xml",
    ]);
    assert_eq!(code, 37, "{out}");
    assert!(!out.contains("invalid_bep"), "{out}");
    let events = json_events(&out);
    let incomplete = events_of_kind(&events, "error")
        .into_iter()
        .find(|event| event["code"] == "incomplete_results")
        .expect("incomplete_results");
    assert!(
        incomplete["message"]
            .as_str()
            .is_some_and(|message| message.contains("build events end early: line 2")),
        "{out}"
    );
    let document: serde_json::Value = serde_json::from_slice(
        &std::fs::read(harness.workspace.join("r.json")).expect("results report"),
    )
    .expect("results json");
    assert_eq!(document["exit_code"], serde_json::json!(37));
    assert_eq!(document["complete"], serde_json::json!(false));
    let results = document["results"].as_array().expect("results");
    assert_eq!(results.len(), 1, "{document}");
    assert_eq!(results[0]["target"], serde_json::json!("//a:t"));
    let junit = std::fs::read_to_string(harness.workspace.join("j.xml")).expect("junit");
    assert!(junit.contains("<testsuite name=\"//a:t\""), "{junit}");
}

#[test]
fn test_cut_off_bep_with_bazel_success_is_operational() {
    let harness = Harness::new("test-cutbep-passed");
    let uri = write_bep_artifact(&harness, "ok.xml", MINIMAL_TEST_XML.as_bytes());
    let harness = Harness {
        unterminated_bep: true,
        raw_bep: Some(vec![
            test_result_line("//a:t", &[(String::from("test.xml"), uri)]),
            String::from(r#"{"id":{"testRes"#),
        ]),
        ..harness
    };
    let (code, _, err) = harness.run(&["test", "--output=text"]);
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("invalid_bep"), "{err}");
}

#[test]
fn coverage_missing_bep_keeps_bazel_code() {
    let mut harness = coverage_harness("cov-nobep-failed", MINIMAL_LCOV.as_bytes());
    harness.skip_bep = true;
    harness.bazel_code = 2;
    let (code, _, err) = harness.run(&["coverage", "--output=text"]);
    assert_eq!(code, 2, "{err}");
    assert!(!err.contains("unreadable_bep"), "{err}");
    assert!(err.contains("build events are unavailable"), "{err}");
}

#[test]
fn test_ignores_non_xml_entries() {
    let harness = Harness::new("test-ignore-log");
    let xml = write_bep_artifact(&harness, "ok.xml", MINIMAL_TEST_XML.as_bytes());
    std::fs::write(harness.temp.join("ok.log"), b"log").expect("log");
    let log_uri = format!("file://{}", harness.temp.join("ok.log").display());
    let harness = Harness {
        raw_bep: Some(vec![test_result_line(
            "//a:t",
            &[
                (String::from("test.log"), log_uri),
                (String::from("test.xml"), xml),
            ],
        )]),
        ..harness
    };
    let (code, _, _) = harness.run(&["test", "--output=text"]);
    assert_eq!(code, 0);
}

#[test]
fn test_unreadable_artifact_is_incomplete() {
    let harness = Harness {
        raw_bep: Some(vec![test_result_line(
            "//a:t",
            &[(String::from("test.xml"), missing_uri("a.xml"))],
        )]),
        ..Harness::new("test-unreadable")
    };
    let (code, _, err) = harness.run(&["test", "--output=text"]);
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("incomplete_results"), "{err}");
}

#[test]
fn test_small_missing_fraction_fails_when_bazel_passed() {
    let harness = Harness::new("test-partial-fraction");
    let raw = (0..6)
        .map(|index| {
            test_result_line(
                &format!("//a:ok{index}"),
                &[(
                    String::from("test.xml"),
                    write_bep_artifact(
                        &harness,
                        &format!("ok5-{index}.xml"),
                        MINIMAL_TEST_XML.as_bytes(),
                    ),
                )],
            )
        })
        .collect::<Vec<_>>();
    let mut lines = raw;
    for index in 0..2 {
        lines.push(test_result_line(
            &format!("//a:missing{index}"),
            &[(
                String::from("test.xml"),
                missing_uri(&format!("fraction{index}.xml")),
            )],
        ));
    }
    let harness = Harness {
        raw_bep: Some(lines),
        ..harness
    };
    let (code, _, err) = harness.run(&["test", "--output=text"]);
    assert_eq!(code, 1, "{err}");
    assert!(
        err.contains("incomplete_results: 2 missing or invalid results"),
        "{err}"
    );
}

#[test]
fn test_large_missing_fraction_fails_when_bazel_passed() {
    let harness = Harness::new("test-partial-majority");
    let raw = (0..2)
        .map(|index| {
            test_result_line(
                &format!("//a:ok{index}"),
                &[(
                    String::from("test.xml"),
                    write_bep_artifact(
                        &harness,
                        &format!("ok6-{index}.xml"),
                        MINIMAL_TEST_XML.as_bytes(),
                    ),
                )],
            )
        })
        .collect::<Vec<_>>();
    let mut lines = raw;
    for index in 0..5 {
        lines.push(test_result_line(
            &format!("//a:missing{index}"),
            &[(
                String::from("test.xml"),
                missing_uri(&format!("majority{index}.xml")),
            )],
        ));
    }
    let harness = Harness {
        raw_bep: Some(lines),
        ..harness
    };
    let (code, _, err) = harness.run(&["test", "--output=text"]);
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("5 missing or invalid results"), "{err}");
}

#[test]
fn test_one_missing_and_one_invalid_xml_fails_when_bazel_passed() {
    let harness = Harness::new("test-mixed-two");
    let raw = (0..6)
        .map(|index| {
            test_result_line(
                &format!("//a:ok{index}"),
                &[(
                    String::from("test.xml"),
                    write_bep_artifact(
                        &harness,
                        &format!("ok4-{index}.xml"),
                        MINIMAL_TEST_XML.as_bytes(),
                    ),
                )],
            )
        })
        .collect::<Vec<_>>();
    let mut lines = raw;
    lines.push(test_result_line(
        "//a:bad",
        &[(
            String::from("test.xml"),
            write_bep_artifact(&harness, "bad2.xml", b"not xml"),
        )],
    ));
    lines.push(test_result_line(
        "//a:missing",
        &[(String::from("test.xml"), missing_uri("missing5.xml"))],
    ));
    let harness = Harness {
        raw_bep: Some(lines),
        ..harness
    };
    let (code, _, err) = harness.run(&["test", "--output=text"]);
    assert_eq!(code, 1, "{err}");
    assert!(
        err.contains("incomplete_results: 2 missing or invalid results"),
        "{err}"
    );
}

#[test]
fn test_single_missing_xml_fails_when_bazel_failed() {
    let harness = Harness::new("test-partial-bazelfail");
    let uri = write_bep_artifact(&harness, "ok2.xml", MINIMAL_TEST_XML.as_bytes());
    let harness = Harness {
        raw_bep: Some(vec![
            test_result_line("//a:ok", &[(String::from("test.xml"), uri)]),
            test_result_line(
                "//a:missing",
                &[(String::from("test.xml"), missing_uri("missing2.xml"))],
            ),
        ]),
        bazel_code: 1,
        ..harness
    };
    let (code, _, err) = harness.run(&["test", "--output=text"]);
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("incomplete_results"), "{err}");
}

#[test]
fn test_invalid_xml_is_incomplete() {
    let harness = Harness::new("test-badxml");
    let uri = write_bep_artifact(&harness, "bad.xml", b"not xml");
    let harness = Harness {
        raw_bep: Some(vec![test_result_line(
            "//a:t",
            &[(String::from("test.xml"), uri)],
        )]),
        ..harness
    };
    let (code, _, err) = harness.run(&["test", "--output=text"]);
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("incomplete_results"), "{err}");
}

#[test]
fn coverage_ignores_non_lcov_and_reports_missing() {
    let harness = Harness::new("cov-ignore");
    let xml = write_bep_artifact(&harness, "x.xml", MINIMAL_TEST_XML.as_bytes());
    let harness = Harness {
        raw_bep: Some(vec![test_result_line(
            "//a:t",
            &[(String::from("test.xml"), xml)],
        )]),
        ..harness
    };
    let (code, _, err) = harness.run(&["coverage", "--output=text"]);
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("no coverage.dat"), "{err}");
}

#[test]
fn coverage_report_without_lcov_names_the_gap_not_the_destination() {
    let harness = Harness::new("cov-gap-report");
    let xml = write_bep_artifact(&harness, "x.xml", MINIMAL_TEST_XML.as_bytes());
    let harness = Harness {
        raw_bep: Some(vec![test_result_line(
            "//a:t",
            &[(String::from("test.xml"), xml)],
        )]),
        ..harness
    };
    let (code, _, err) = harness.run(&["coverage", "--output=text", "--report=lcov=out.lcov"]);
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("report_failed"), "{err}");
    assert!(err.contains("no coverage.dat"), "{err}");
    assert!(err.contains("fix the test or coverage problem"), "{err}");
    assert!(!err.contains("writable"), "{err}");
}

#[test]
fn coverage_unreadable_is_incomplete() {
    let harness = Harness {
        raw_bep: Some(vec![test_result_line(
            "//a:t",
            &[(String::from("coverage.dat"), missing_uri("c.dat"))],
        )]),
        ..Harness::new("cov-unreadable")
    };
    let (code, _, err) = harness.run(&["coverage", "--output=text"]);
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("incomplete_results"), "{err}");
}

#[test]
fn coverage_single_missing_dat_fails_when_bazel_passed() {
    let harness = Harness::new("cov-partial-missing");
    let raw = (0..4)
        .map(|index| {
            test_result_line(
                &format!("//a:ok{index}"),
                &[(
                    String::from("coverage.dat"),
                    write_bep_artifact(
                        &harness,
                        &format!("ok7-{index}.dat"),
                        MINIMAL_LCOV.as_bytes(),
                    ),
                )],
            )
        })
        .collect::<Vec<_>>();
    let mut lines = raw;
    lines.push(test_result_line(
        "//a:missing",
        &[(String::from("coverage.dat"), missing_uri("missing.dat"))],
    ));
    let harness = Harness {
        raw_bep: Some(lines),
        ..harness
    };
    let (code, _, err) = harness.run(&["coverage", "--output=text"]);
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("incomplete_results"), "{err}");
    assert!(
        err.contains("coverage.dat for //a:missing (run 1, shard 1, attempt 1)"),
        "{err}"
    );
}

#[test]
fn coverage_missing_artifact_blocks_min_coverage() {
    let harness = Harness::new("cov-missing-threshold");
    let raw = (0..4)
        .map(|index| {
            test_result_line(
                &format!("//a:ok{index}"),
                &[(
                    String::from("coverage.dat"),
                    write_bep_artifact(
                        &harness,
                        &format!("ok8-{index}.dat"),
                        MINIMAL_LCOV.as_bytes(),
                    ),
                )],
            )
        })
        .collect::<Vec<_>>();
    let mut lines = raw;
    lines.push(test_result_line(
        "//a:missing",
        &[(String::from("coverage.dat"), missing_uri("missing.dat"))],
    ));
    let harness = Harness {
        raw_bep: Some(lines),
        ..harness
    };
    let (code, _, err) = harness.run(&["coverage", "--output=text", "--min-coverage=100"]);
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("incomplete_results"), "{err}");
    assert!(!err.contains("meets minimum"), "{err}");
}

#[test]
fn coverage_invalid_artifact_blocks_min_coverage() {
    let harness = Harness::new("cov-invalid-threshold");
    let ok_uri = write_bep_artifact(&harness, "ok.dat", MINIMAL_LCOV.as_bytes());
    let bad_uri = write_bep_artifact(&harness, "bad.dat", b"not lcov");
    let harness = Harness {
        raw_bep: Some(vec![
            test_result_line("//a:ok", &[(String::from("coverage.dat"), ok_uri)]),
            test_result_line("//a:bad", &[(String::from("coverage.dat"), bad_uri)]),
        ]),
        ..harness
    };
    let (code, _, err) = harness.run(&["coverage", "--output=text", "--min-coverage=100"]);
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("incomplete_results"), "{err}");
    assert!(
        err.contains("invalid coverage.dat for //a:bad (run 1, shard 1, attempt 1)"),
        "{err}"
    );
    assert!(!err.contains("meets minimum"), "{err}");
}

#[test]
fn coverage_superseded_attempt_is_not_required() {
    let harness = Harness::new("cov-retry-ok");
    let final_uri = write_bep_artifact(&harness, "final.dat", MINIMAL_LCOV.as_bytes());
    let harness = Harness {
        raw_bep: Some(vec![
            test_result_identity_line(
                "//a:t",
                1,
                1,
                1,
                &[(String::from("test.lcov"), missing_uri("attempt1.dat"))],
            ),
            test_result_identity_line("//a:t", 1, 1, 2, &[(String::from("test.lcov"), final_uri)]),
        ]),
        ..harness
    };
    let harness = owning_a(harness);
    let (code, _, err) = harness.run(&["coverage", "--output=text", "--min-coverage=100"]);
    assert_eq!(code, 0, "{err}");
    assert!(!err.contains("incomplete_results"), "{err}");
    assert!(err.contains("meets minimum 100%"), "{err}");
}

#[test]
fn coverage_final_attempt_missing_is_required() {
    let harness = Harness::new("cov-retry-missing");
    let stale_uri = write_bep_artifact(&harness, "stale.dat", MINIMAL_LCOV.as_bytes());
    let harness = Harness {
        raw_bep: Some(vec![
            test_result_identity_line("//a:t", 1, 1, 1, &[(String::from("test.lcov"), stale_uri)]),
            test_result_identity_line(
                "//a:t",
                1,
                1,
                2,
                &[(String::from("test.lcov"), missing_uri("final.dat"))],
            ),
        ]),
        ..harness
    };
    let (code, _, err) = harness.run(&["coverage", "--output=text"]);
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("incomplete_results"), "{err}");
    assert!(
        err.contains("test.lcov for //a:t (run 1, shard 1, attempt 2)"),
        "{err}"
    );
}

#[test]
fn coverage_uninstrumented_test_does_not_demand_coverage() {
    let harness = Harness::new("cov-uninstrumented");
    let lcov_uri = write_bep_artifact(&harness, "inst.dat", MINIMAL_LCOV.as_bytes());
    let xml_uri = write_bep_artifact(&harness, "plain.xml", MINIMAL_TEST_XML.as_bytes());
    let harness = Harness {
        raw_bep: Some(vec![
            test_result_line("//a:instrumented", &[(String::from("test.lcov"), lcov_uri)]),
            test_result_line("//a:plain", &[(String::from("test.xml"), xml_uri)]),
        ]),
        ..harness
    };
    let harness = owning_a(harness);
    let (code, _, err) = harness.run(&["coverage", "--output=text", "--min-coverage=100"]);
    assert_eq!(code, 0, "{err}");
    assert!(!err.contains("incomplete_results"), "{err}");
    assert!(err.contains("meets minimum 100%"), "{err}");
}

#[test]
fn coverage_partial_report_is_written_and_marked_incomplete() {
    let harness = Harness::new("cov-partial-report");
    let ok_uri = write_bep_artifact(&harness, "ok.dat", MINIMAL_LCOV.as_bytes());
    let harness = Harness {
        raw_bep: Some(vec![
            test_result_line("//a:ok", &[(String::from("coverage.dat"), ok_uri)]),
            test_result_line(
                "//a:missing",
                &[(String::from("coverage.dat"), missing_uri("missing.dat"))],
            ),
        ]),
        ..harness
    };
    let (code, out, err) = harness.run(&["coverage", "--output=text", "--report=lcov=out.lcov"]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(out.contains("Wrote lcov report to out.lcov."), "{out}");
    assert!(err.contains("incomplete_results"), "{err}");
    assert!(
        err.contains("coverage.dat for //a:missing"),
        "the partial report names the missing target artifact: {err}"
    );
    let document = std::fs::read(harness.workspace.join("out.lcov")).expect("lcov");
    assert_eq!(document, MINIMAL_LCOV.as_bytes());
}

#[test]
fn coverage_incomplete_json_agrees_with_exit_status() {
    let harness = Harness::new("cov-incomplete-json");
    let ok_uri = write_bep_artifact(&harness, "ok.dat", MINIMAL_LCOV.as_bytes());
    let harness = Harness {
        raw_bep: Some(vec![
            test_result_line("//a:ok", &[(String::from("coverage.dat"), ok_uri)]),
            test_result_line(
                "//a:missing",
                &[(String::from("coverage.dat"), missing_uri("missing.dat"))],
            ),
        ]),
        ..harness
    };
    let (code, out, _) = harness.run(&["coverage", "--output=json", "--min-coverage=100"]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("incomplete_results"), "{out}");
    assert!(out.contains("\"results_complete\":false"), "{out}");
    assert!(!out.contains("coverage_below_minimum"), "{out}");
}

#[test]
fn coverage_two_missing_dats_fail_when_bazel_passed() {
    let harness = Harness::new("cov-partial-two");
    let raw = (0..6)
        .map(|index| {
            test_result_line(
                &format!("//a:ok{index}"),
                &[(
                    String::from("coverage.dat"),
                    write_bep_artifact(
                        &harness,
                        &format!("ok3-{index}.dat"),
                        MINIMAL_LCOV.as_bytes(),
                    ),
                )],
            )
        })
        .collect::<Vec<_>>();
    let mut lines = raw;
    for index in 0..2 {
        lines.push(test_result_line(
            &format!("//a:missing{index}"),
            &[(
                String::from("coverage.dat"),
                missing_uri(&format!("missing3-{index}.dat")),
            )],
        ));
    }
    let harness = Harness {
        raw_bep: Some(lines),
        ..harness
    };
    let (code, _, err) = harness.run(&["coverage", "--output=text"]);
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("incomplete_results"), "{err}");
    assert!(err.contains("2 missing or invalid results"), "{err}");
}

#[test]
fn coverage_one_invalid_and_one_missing_dat_fail_when_bazel_passed() {
    let harness = Harness::new("cov-mixed-two");
    let raw = (0..6)
        .map(|index| {
            test_result_line(
                &format!("//a:ok{index}"),
                &[(
                    String::from("test.lcov"),
                    write_bep_artifact(
                        &harness,
                        &format!("ok4-{index}.dat"),
                        MINIMAL_LCOV.as_bytes(),
                    ),
                )],
            )
        })
        .collect::<Vec<_>>();
    let mut lines = raw;
    lines.push(test_result_line(
        "//a:bad",
        &[(
            String::from("test.lcov"),
            write_bep_artifact(&harness, "bad3.dat", b"not lcov"),
        )],
    ));
    lines.push(test_result_line(
        "//a:missing",
        &[(String::from("coverage.dat"), missing_uri("missing5.dat"))],
    ));
    let harness = Harness {
        raw_bep: Some(lines),
        ..harness
    };
    let (code, _, err) = harness.run(&["coverage", "--output=text"]);
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("incomplete_results"), "{err}");
    assert!(err.contains("2 missing or invalid results"), "{err}");
}

#[test]
fn coverage_large_missing_fraction_fails_when_bazel_passed() {
    let harness = Harness::new("cov-partial-majority");
    let raw = (0..2)
        .map(|index| {
            test_result_line(
                &format!("//a:ok{index}"),
                &[(
                    String::from("coverage.dat"),
                    write_bep_artifact(
                        &harness,
                        &format!("ok6-{index}.dat"),
                        MINIMAL_LCOV.as_bytes(),
                    ),
                )],
            )
        })
        .collect::<Vec<_>>();
    let mut lines = raw;
    for index in 0..5 {
        lines.push(test_result_line(
            &format!("//a:missing{index}"),
            &[(
                String::from("coverage.dat"),
                missing_uri(&format!("covmajority{index}.dat")),
            )],
        ));
    }
    let harness = Harness {
        raw_bep: Some(lines),
        ..harness
    };
    let (code, _, err) = harness.run(&["coverage", "--output=text"]);
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("5 missing or invalid results"), "{err}");
}

#[test]
fn coverage_empty_dat_does_not_count_as_missing() {
    let harness = Harness::new("cov-empty-ok");
    let empty = write_bep_artifact(&harness, "empty2.dat", b"");
    let ok = write_bep_artifact(&harness, "ok5.dat", MINIMAL_LCOV.as_bytes());
    let harness = Harness {
        raw_bep: Some(vec![
            test_result_line("//a:empty", &[(String::from("coverage.dat"), empty)]),
            test_result_line("//a:ok", &[(String::from("coverage.dat"), ok)]),
        ]),
        ..harness
    };
    let (code, _, err) = harness.run(&["coverage", "--output=text"]);
    assert_eq!(code, 0, "{err}");
    assert!(!err.contains("incomplete_results"), "{err}");
}

#[test]
fn coverage_single_missing_dat_fails_when_bazel_failed() {
    let harness = Harness::new("cov-partial-bazelfail");
    let uri = write_bep_artifact(&harness, "ok2.dat", MINIMAL_LCOV.as_bytes());
    let harness = Harness {
        raw_bep: Some(vec![
            test_result_line("//a:ok", &[(String::from("coverage.dat"), uri)]),
            test_result_line(
                "//a:missing",
                &[(String::from("coverage.dat"), missing_uri("missing2.dat"))],
            ),
        ]),
        bazel_code: 1,
        ..harness
    };
    let (code, _, err) = harness.run(&["coverage", "--output=text"]);
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("incomplete_results"), "{err}");
}

#[test]
fn coverage_report_failed_when_incomplete() {
    let harness = Harness::new("cov-repfail");
    let uri = write_bep_artifact(&harness, "bad2.dat", b"not lcov");
    let harness = Harness {
        raw_bep: Some(vec![test_result_line(
            "//a:t",
            &[(String::from("test.lcov"), uri)],
        )]),
        ..harness
    };
    let (code, _, err) = harness.run(&["coverage", "--output=text", "--report=lcov=out.lcov"]);
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("report_failed"), "{err}");
}

#[test]
fn test_report_write_failure_is_operational() {
    let harness = Harness::new("test-writefail");
    let uri = write_bep_artifact(&harness, "w.xml", MINIMAL_TEST_XML.as_bytes());
    let harness = Harness {
        raw_bep: Some(vec![test_result_line(
            "//a:t",
            &[(String::from("test.xml"), uri)],
        )]),
        ..harness
    };
    let (code, _, err) = harness.run(&[
        "test",
        "--output=text",
        "--report=junit=missing-dir/out.xml",
    ]);
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("report_failed"), "{err}");
}

#[test]
fn test_report_json_emits_report_event() {
    let harness = Harness::new("test-jsonrep");
    let uri = write_bep_artifact(&harness, "j.xml", MINIMAL_TEST_XML.as_bytes());
    let harness = Harness {
        raw_bep: Some(vec![test_result_line(
            "//a:t",
            &[(String::from("test.xml"), uri)],
        )]),
        ..harness
    };
    let (code, out, _) = harness.run(&["test", "--output=json", "--report=junit=out.xml"]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("report"), "{out}");
    assert!(out.contains("command_finished"), "{out}");
}

#[test]
fn test_rejects_diff_output_at_parse() {
    let err = dx_plan::args::parse(
        &["test", "--diff", "--report=junit=out.xml"]
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
    )
    .expect_err("diff rejected");
    assert_eq!(
        err,
        dx_plan::args::ArgsError::UnsupportedOption {
            command: "test",
            option: "--diff".to_owned(),
        }
    );
}

#[test]
fn test_incomplete_json_reports_incomplete_event() {
    let harness = Harness {
        raw_bep: Some(vec![String::from(
            "{\"id\": {\"testResult\": {\"label\": \"//a:t\"}}, \"testResult\": {\"status\": \"PASSED\"}}",
        )]),
        ..Harness::new("test-incjson")
    };
    let (code, out, _) = harness.run(&["test", "--output=json"]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("incomplete_results"), "{out}");
    assert!(out.contains("command_finished"), "{out}");
}

#[test]
fn coverage_lcov_without_trailing_newline_gets_newline() {
    let harness = Harness::new("cov-nonl");
    let raw = b"SF:src/a.py\nDA:1,1\nend_of_record";
    let uri = write_bep_artifact(&harness, "nonl.dat", raw);
    let harness = Harness {
        raw_bep: Some(vec![test_result_line(
            "//a:t",
            &[(String::from("test.lcov"), uri)],
        )]),
        ..harness
    };
    let (code, out, _) = harness.run(&["coverage", "--output=text", "--report=lcov=out.lcov"]);
    assert_eq!(code, 0, "{out}");
    let document = std::fs::read(harness.workspace.join("out.lcov")).expect("lcov");
    assert!(document.ends_with(b"\n"), "{document:?}");
}

#[test]
fn coverage_render_failure_json_reports_error_event() {
    let harness = Harness::new("cov-render-json");
    let xml = write_bep_artifact(&harness, "x.xml", MINIMAL_TEST_XML.as_bytes());
    let harness = Harness {
        raw_bep: Some(vec![test_result_line(
            "//a:t",
            &[(String::from("test.xml"), xml)],
        )]),
        ..harness
    };
    let (code, out, _) = harness.run(&["coverage", "--output=json", "--report=lcov=out.lcov"]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("report_failed"), "{out}");
}

#[test]
fn test_report_write_failure_json_reports_error_event() {
    let harness = Harness::new("test-writefail-json");
    let uri = write_bep_artifact(&harness, "w2.xml", MINIMAL_TEST_XML.as_bytes());
    let harness = Harness {
        raw_bep: Some(vec![test_result_line(
            "//a:t",
            &[(String::from("test.xml"), uri)],
        )]),
        ..harness
    };
    let (code, out, _) = harness.run(&[
        "test",
        "--output=json",
        "--report=junit=missing-dir/out.xml",
    ]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("report_failed"), "{out}");
}

#[test]
fn a_testlogs_path_keeps_its_package_and_identity() {
    let execroot = "/home/someone/.cache/bazel/_bazel_x/abc123/execroot/_main";
    assert_eq!(
        stable_exec_path(Path::new(&format!(
            "{execroot}/bazel-out/k8-fastbuild/testlogs/cli/bep/dx_bep_test/run_2_of_2/test.xml"
        ))),
        "cli/bep/dx_bep_test/run_2_of_2/test.xml"
    );
    assert_eq!(
        stable_exec_path(Path::new("/ws/bazel-testlogs/a/t/shard_1_of_2/test.xml")),
        "a/t/shard_1_of_2/test.xml"
    );
}

#[test]
fn test_file_outputs_skip_bazel_info() {
    let harness = Harness::new("test-file-no-info");
    let uri = write_bep_artifact(&harness, "test.xml", MINIMAL_TEST_XML.as_bytes());
    let harness = Harness {
        raw_bep: Some(vec![test_result_line(
            "//a:t",
            &[(String::from("test.xml"), uri)],
        )]),
        ..harness
    };
    let (code, out, err) = harness.run(&["test", "--output=text", "--report=junit=out.xml"]);
    assert_eq!(code, 0, "{out}{err}");
    let document = std::fs::read(harness.workspace.join("out.xml")).expect("junit");
    let text = String::from_utf8(document).expect("utf8");
    assert!(text.contains("tests=\"1\""), "{text}");
    assert!(harness.query.info_calls.borrow().is_empty());
}

#[test]
fn test_bytestream_outputs_resolve_through_info_testlogs() {
    let harness = Harness::new("test-info-roots");
    let testlogs = harness.temp.join("testlogs");
    harness.query.script_info(&format!(
        "bazel-testlogs: {}\nexecution_root: {}\nignored: {}\n",
        testlogs.display(),
        harness.temp.join("execroot").display(),
        harness.temp.display(),
    ));
    let root = testlogs.join("a").join("t");
    std::fs::create_dir_all(&root).expect("testlogs dir");
    std::fs::write(root.join("test.xml"), MINIMAL_TEST_XML).expect("test.xml");
    let harness = Harness {
        raw_bep: Some(vec![test_result_line(
            "//a:t",
            &[(
                String::from("test.xml"),
                "bytestream://remote.buildbuddy.io/blobs/abc/10".to_owned(),
            )],
        )]),
        ..harness
    };
    let (code, out, err) = harness.run(&["test", "--output=text", "--report=junit=out.xml"]);
    assert_eq!(code, 0, "{out}{err}");
    let document = std::fs::read(harness.workspace.join("out.xml")).expect("junit");
    let text = String::from_utf8(document).expect("utf8");
    assert!(text.contains("tests=\"1\""), "{text}");
    let calls = harness.query.info_calls.borrow();
    assert_eq!(calls.len(), 1, "one bazel info run per report run");
    let argv = &calls[0];
    assert!(argv.contains(&"info".to_owned()), "{argv:?}");
    assert!(argv.contains(&"bazel-testlogs".to_owned()), "{argv:?}");
    assert!(argv.contains(&"execution_root".to_owned()), "{argv:?}");
    assert!(argv.contains(&"output_path".to_owned()), "{argv:?}");
    assert!(
        argv.contains(&"--config=dx_build_dev".to_owned()),
        "the test profile reaches info: {argv:?}"
    );
    assert!(
        harness.query.calls.borrow().is_empty(),
        "info must not consume query outputs"
    );
}

#[test]
fn test_bytestream_outputs_resolve_through_configuration_mnemonic() {
    let harness = Harness::new("test-info-mnemonic");
    let out = harness.temp.join("out");
    harness.query.script_info(&format!(
        "bazel-testlogs: {}\nexecution_root: {}\noutput_path: {}\n",
        out.join("k8-fastbuild").join("testlogs").display(),
        harness.temp.join("execroot").display(),
        out.display(),
    ));
    let root = out
        .join("k8-fastbuild-ST-x")
        .join("testlogs")
        .join("a")
        .join("t");
    std::fs::create_dir_all(&root).expect("testlogs dir");
    std::fs::write(root.join("test.xml"), MINIMAL_TEST_XML).expect("test.xml");
    let configuration = serde_json::json!({
        "id": {"configuration": {"id": "cfgST"}},
        "configuration": {"mnemonic": "k8-fastbuild-ST-x"},
    })
    .to_string();
    let harness = Harness {
        raw_bep: Some(vec![
            configuration,
            test_result_full_line(
                "//a:t",
                1,
                1,
                1,
                "PASSED",
                Some("cfgST"),
                None,
                None,
                &[(
                    String::from("test.xml"),
                    "bytestream://remote.buildbuddy.io/blobs/abc/10".to_owned(),
                )],
            ),
        ]),
        ..harness
    };
    let (code, out, err) = harness.run(&["test", "--output=json", "--report=junit=out.xml"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(!out.contains("incomplete_results"), "{out}");
    let document = std::fs::read(harness.workspace.join("out.xml")).expect("junit");
    let text = String::from_utf8(document).expect("utf8");
    assert!(text.contains("tests=\"1\""), "{text}");
}

#[test]
fn test_info_carries_startup_options_before_the_verb() {
    let harness = Harness::new("test-info-startup");
    let testlogs = harness.temp.join("testlogs");
    harness.query.script_info(&format!(
        "bazel-testlogs: {}\nexecution_root: {}\n",
        testlogs.display(),
        harness.temp.join("execroot").display(),
    ));
    let root = testlogs.join("a").join("t");
    std::fs::create_dir_all(&root).expect("testlogs dir");
    std::fs::write(root.join("test.xml"), MINIMAL_TEST_XML).expect("test.xml");
    let harness = Harness {
        raw_bep: Some(vec![test_result_line(
            "//a:t",
            &[(
                String::from("test.xml"),
                "bytestream://remote.buildbuddy.io/blobs/abc/10".to_owned(),
            )],
        )]),
        ..harness
    };
    let (code, _, err) = harness.run(&[
        "test",
        "--bazel-startup-option=--output_base=/tmp/a",
        "--output=text",
    ]);
    assert_eq!(code, 0, "{err}");
    let calls = harness.query.info_calls.borrow();
    assert_eq!(calls.len(), 1, "one bazel info run per report run");
    let argv = &calls[0];
    let verb = argv.iter().position(|arg| arg == "info").expect("info");
    assert_eq!(
        &argv[..verb],
        &[
            "bazel".to_owned(),
            "--nohome_rc".to_owned(),
            "--nosystem_rc".to_owned(),
            "--output_base=/tmp/a".to_owned(),
        ]
    );
}

#[test]
fn test_sharded_and_retried_bytestream_outputs_keep_identity() {
    let harness = Harness::new("test-info-identity");
    let testlogs = harness.temp.join("testlogs");
    harness
        .query
        .script_info(&format!("bazel-testlogs: {}\n", testlogs.display()));
    let unit = testlogs.join("a").join("t");
    let attempts = unit.join("shard_1_of_2").join("test_attempts");
    std::fs::create_dir_all(&attempts).expect("attempt dir");
    std::fs::create_dir_all(unit.join("shard_2_of_2")).expect("shard dir");
    std::fs::write(attempts.join("attempt_1.xml"), MINIMAL_TEST_XML).expect("attempt xml");
    std::fs::write(unit.join("shard_1_of_2").join("test.xml"), MINIMAL_TEST_XML)
        .expect("shard xml");
    std::fs::write(unit.join("shard_2_of_2").join("test.xml"), MINIMAL_TEST_XML)
        .expect("shard xml");
    let entry = || {
        vec![(
            String::from("test.xml"),
            "bytestream://remote.buildbuddy.io/blobs/abc/10".to_owned(),
        )]
    };
    let harness = Harness {
        raw_bep: Some(vec![
            test_summary_line("//a:t", 2),
            test_result_identity_line("//a:t", 1, 1, 1, &entry()),
            test_result_identity_line("//a:t", 1, 1, 2, &entry()),
            test_result_identity_line("//a:t", 1, 2, 1, &entry()),
        ]),
        ..harness
    };
    let (code, out, err) = harness.run(&["test", "--output=text", "--report=junit=out.xml"]);
    assert_eq!(code, 0, "{out}{err}");
    let document = std::fs::read(harness.workspace.join("out.xml")).expect("junit");
    let text = String::from_utf8(document).expect("utf8");
    assert!(text.contains("tests=\"3\""), "{text}");
}

#[test]
fn two_runs_of_one_label_render_distinct_case_names() {
    let harness = Harness::new("test-junit-runs");
    let first = write_bep_artifact(&harness, "run1.xml", MINIMAL_TEST_XML.as_bytes());
    let second = write_bep_artifact(&harness, "run2.xml", MINIMAL_TEST_XML.as_bytes());
    let harness = Harness {
        raw_bep: Some(vec![
            test_result_identity_line("//a:t", 1, 1, 1, &[(String::from("test.xml"), first)]),
            test_result_identity_line("//a:t", 2, 1, 1, &[(String::from("test.xml"), second)]),
        ]),
        ..harness
    };
    let (code, out, err) = harness.run(&["test", "--output=text", "--report=junit=out.xml"]);
    assert_eq!(code, 0, "{out}{err}");
    let text = std::fs::read_to_string(harness.workspace.join("out.xml")).expect("junit");
    assert!(
        text.contains(r#"<testcase name="passes [run=0]""#),
        "{text}"
    );
    assert!(
        text.contains(r#"<testcase name="passes [run=1]""#),
        "{text}"
    );
}

#[test]
fn invalid_utf8_test_xml_still_counts() {
    let harness = Harness::new("test-junit-lossy");
    let xml = b"<testsuites><testsuite name=\"s\"><testcase name=\"raw\"><system-out>\xff</system-out></testcase></testsuite></testsuites>";
    let uri = write_bep_artifact(&harness, "test.xml", xml);
    let harness = Harness {
        raw_bep: Some(vec![test_result_line(
            "//a:t",
            &[(String::from("test.xml"), uri)],
        )]),
        ..harness
    };
    let (code, out, err) = harness.run(&["test", "--output=text", "--report=junit=out.xml"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(
        err.contains(
            "dx: warning: test.xml for //a:t (run 1, shard 1, attempt 1) has invalid UTF-8"
        ),
        "{err}"
    );
    let text = std::fs::read_to_string(harness.workspace.join("out.xml")).expect("junit");
    assert!(text.contains(r#"<testcase name="raw""#), "{text}");
}

#[test]
fn test_unreadable_bytestream_names_label_and_identity() {
    let harness = Harness::new("test-info-missing");
    harness.query.script_info(&format!(
        "bazel-testlogs: {}\n",
        harness.temp.join("testlogs").display()
    ));
    let harness = Harness {
        raw_bep: Some(vec![test_result_identity_line(
            "//a:t",
            2,
            3,
            1,
            &[(
                String::from("test.xml"),
                "bytestream://remote.buildbuddy.io/blobs/abc/10".to_owned(),
            )],
        )]),
        ..harness
    };
    let (code, _, err) = harness.run(&["test", "--output=text"]);
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("incomplete_results"), "{err}");
    assert!(
        err.contains("unreadable test.xml for //a:t (run 2, shard 3, attempt 1)"),
        "{err}"
    );
    assert!(
        err.contains("a/t/shard_3_of_3_run_2_of_2/test.xml"),
        "the stable path keeps package and identity: {err}"
    );
}

#[test]
fn test_workspace_fallback_survives_without_info() {
    let harness = Harness::new("test-no-info");
    let root = harness.workspace.join("bazel-testlogs").join("a").join("t");
    std::fs::create_dir_all(&root).expect("testlogs dir");
    std::fs::write(root.join("test.xml"), MINIMAL_TEST_XML).expect("test.xml");
    let harness = Harness {
        raw_bep: Some(vec![test_result_line(
            "//a:t",
            &[(
                String::from("test.xml"),
                "bytestream://remote.buildbuddy.io/blobs/abc/10".to_owned(),
            )],
        )]),
        ..harness
    };
    let (code, out, err) = harness.run(&["test", "--output=text", "--report=junit=out.xml"]);
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(harness.query.info_calls.borrow().len(), 1);
}

const MIXED_TEST_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?><testsuites><testsuite name="s"><testcase name="passes" classname="c" time="0.1"/><testcase name="fails" classname="c" time="0.2"><failure message="m">t</failure></testcase><testcase name="errors" classname="c" time="0.3"><error message="e">t</error></testcase><testcase name="skips" classname="c" time="0"><skipped message="s"/></testcase></testsuite></testsuites>"#;

const EMPTY_TEST_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?><testsuites><testsuite name="s"></testsuite></testsuites>"#;

#[test]
fn test_outcomes_carry_bep_status_cache_and_timing() {
    let harness = Harness::new("test-outcome-meta");
    let ok_uri = write_bep_artifact(&harness, "ok.xml", MINIMAL_TEST_XML.as_bytes());
    let cached_uri = write_bep_artifact(&harness, "cached.xml", MINIMAL_TEST_XML.as_bytes());
    let harness = Harness {
        raw_bep: Some(vec![
            test_result_full_line(
                "//a:ok",
                1,
                1,
                1,
                "PASSED",
                Some("cfg-one"),
                Some(false),
                Some(12),
                &[(String::from("test.xml"), ok_uri)],
            ),
            test_result_full_line(
                "//a:cached",
                2,
                3,
                1,
                "FAILED",
                None,
                Some(true),
                None,
                &[(String::from("test.xml"), cached_uri)],
            ),
        ]),
        ..harness
    };
    let (code, out, _) = harness.run(&["test", "--output=json"]);
    assert_eq!(code, 0, "{out}");
    let events = json_events(&out);
    let outcomes = events_of_kind(&events, "test_outcome");
    assert_eq!(outcomes.len(), 1, "{out}");
    assert_eq!(outcomes[0]["target"], serde_json::json!("//a:cached"));
    assert_eq!(outcomes[0]["outcome"], serde_json::json!("failed"));
    assert_eq!(outcomes[0]["cached"], serde_json::json!(true));
    assert_eq!(outcomes[0]["run"], serde_json::json!(2));
    assert_eq!(outcomes[0]["shard"], serde_json::json!(3));
    assert!(outcomes[0].get("configuration").is_none(), "{out}");
    assert!(outcomes[0].get("duration_millis").is_none(), "{out}");
    let finished_event = event(&events, "command_finished");
    assert_eq!(
        finished_event["tests"],
        serde_json::json!({"passed": 1, "failed": 1, "cached": 0, "no_cases": 0, "skipped": 0})
    );
    assert_eq!(finished_event["exit_code"], serde_json::json!(0));
    assert_eq!(finished_event["results_complete"], serde_json::json!(true));
}

#[test]
fn the_passed_count_matches_the_results_report() {
    let harness = Harness::new("test-outcome-all-passed");
    let mut lines = Vec::new();
    for index in 0..3 {
        let uri = write_bep_artifact(
            &harness,
            &format!("cases-{index}.xml"),
            MINIMAL_TEST_XML.as_bytes(),
        );
        lines.push(test_result_line(
            &format!("//rust:compile_test{index}"),
            &[(String::from("test.xml"), uri)],
        ));
    }
    for index in 0..4 {
        let uri = write_bep_artifact(
            &harness,
            &format!("empty-{index}.xml"),
            EMPTY_TEST_XML.as_bytes(),
        );
        lines.push(test_result_line(
            &format!("//go:generation_test{index}"),
            &[(String::from("test.xml"), uri)],
        ));
    }
    let harness = Harness {
        raw_bep: Some(lines),
        ..harness
    };
    let (code, out, _) = harness.run(&["test", "--output=json", "--report=results=res.json"]);
    assert_eq!(code, 0, "{out}");
    let events = json_events(&out);
    let outcomes = events_of_kind(&events, "test_outcome");
    assert!(outcomes.is_empty(), "{out}");
    let results: serde_json::Value = serde_json::from_slice(
        &std::fs::read(harness.workspace.join("res.json")).expect("results report"),
    )
    .expect("results json");
    let entries = results["results"].as_array().expect("results");
    assert_eq!(entries.len(), 7, "{results}");
    assert!(
        entries
            .iter()
            .all(|entry| entry["bazel_status"] == "PASSED"),
        "{results}"
    );
    let no_cases = entries
        .iter()
        .filter(|entry| entry["outcome"] == "no_cases")
        .count();
    assert_eq!(no_cases, 4, "{results}");
    assert_eq!(
        event(&events, "command_finished")["tests"],
        serde_json::json!({"passed": 7, "failed": 0, "cached": 0, "no_cases": 4, "skipped": 0})
    );
}

#[test]
fn test_outcome_counts_cases_by_severity() {
    let harness = Harness::new("test-outcome-cases");
    let uri = write_bep_artifact(&harness, "mixed.xml", MIXED_TEST_XML.as_bytes());
    let harness = Harness {
        raw_bep: Some(vec![test_result_full_line(
            "//a:t",
            1,
            1,
            1,
            "FAILED",
            None,
            None,
            None,
            &[(String::from("test.xml"), uri)],
        )]),
        ..harness
    };
    let (code, out, _) = harness.run(&["test", "--output=json"]);
    assert_eq!(code, 0, "{out}");
    let events = json_events(&out);
    let outcomes = events_of_kind(&events, "test_outcome");
    assert_eq!(outcomes.len(), 1, "{out}");
    assert_eq!(outcomes[0]["outcome"], serde_json::json!("failed"));
    assert_eq!(outcomes[0]["cases_total"], serde_json::json!(4));
    assert_eq!(outcomes[0]["cases_failed"], serde_json::json!(1));
    assert_eq!(outcomes[0]["cases_error"], serde_json::json!(1));
    assert_eq!(outcomes[0]["cases_skipped"], serde_json::json!(1));
    assert!(outcomes[0].get("artifacts").is_none(), "{out}");
    assert!(outcomes[0].get("evidence_complete").is_none(), "{out}");
    let failing = outcomes[0]["failing_cases"].as_array().expect("failing");
    let names: Vec<_> = failing.iter().map(|case| case["name"].clone()).collect();
    assert_eq!(
        names,
        vec![serde_json::json!("fails"), serde_json::json!("errors")]
    );
}

#[test]
fn test_flaky_result_reports_passed_after_retry() {
    let harness = Harness::new("test-outcome-flaky");
    let first_uri = write_bep_artifact(&harness, "first.xml", MINIMAL_TEST_XML.as_bytes());
    let retry_uri = write_bep_artifact(&harness, "retry.xml", MINIMAL_TEST_XML.as_bytes());
    let harness = Harness {
        raw_bep: Some(vec![
            test_result_full_line(
                "//a:t",
                1,
                1,
                1,
                "FAILED",
                None,
                Some(false),
                None,
                &[(String::from("test.xml"), first_uri)],
            ),
            test_result_full_line(
                "//a:t",
                1,
                1,
                2,
                "FLAKY",
                None,
                Some(false),
                None,
                &[(String::from("test.xml"), retry_uri)],
            ),
        ]),
        ..harness
    };
    let (code, out, _) = harness.run(&["test", "--output=json", "--report=results=res.json"]);
    assert_eq!(code, 0, "{out}");
    let events = json_events(&out);
    let outcomes = events_of_kind(&events, "test_outcome");
    assert_eq!(outcomes.len(), 2, "{out}");
    assert_eq!(outcomes[0]["outcome"], serde_json::json!("failed"));
    assert_eq!(outcomes[0]["attempt"], serde_json::json!(1));
    assert_eq!(outcomes[0]["final"], serde_json::json!(false), "{out}");
    assert_eq!(
        outcomes[1]["outcome"],
        serde_json::json!("passed_after_retry")
    );
    assert_eq!(outcomes[1]["attempt"], serde_json::json!(2));
    assert!(outcomes[1].get("final").is_none(), "{out}");
    assert_eq!(
        event(&events, "command_finished")["tests"],
        serde_json::json!({"passed": 1, "failed": 0, "cached": 0, "no_cases": 0, "skipped": 0})
    );
    let read_results = || -> serde_json::Value {
        serde_json::from_slice(
            &std::fs::read(harness.workspace.join("res.json")).expect("results report"),
        )
        .expect("results json")
    };
    let results = read_results();
    let entries = results["results"].as_array().expect("results");
    assert_eq!(entries.len(), 2, "{results}");
    assert_eq!(entries[0]["attempt"], serde_json::json!(1));
    assert_eq!(entries[0]["final"], serde_json::json!(false), "{results}");
    assert_eq!(entries[1]["attempt"], serde_json::json!(2));
    assert!(entries[1].get("final").is_none(), "{results}");
    let (code, out, err) = harness.run(&["test", "--output=text", "--report=results=res.json"]);
    assert_eq!(code, 0, "{out}{err}");
    let results = read_results();
    let entries = results["results"].as_array().expect("results");
    assert_eq!(entries.len(), 2, "{results}");
    assert_eq!(entries[0]["attempt"], serde_json::json!(1));
    assert_eq!(entries[0]["final"], serde_json::json!(false), "{results}");
    assert!(entries[1].get("final").is_none(), "{results}");
}

#[test]
fn test_flaky_summary_marks_retried_pass_distinctly() {
    let harness = Harness::new("test-summary-flaky");
    let first_uri = write_bep_artifact(&harness, "first.xml", MINIMAL_TEST_XML.as_bytes());
    let retry_uri = write_bep_artifact(&harness, "retry.xml", MINIMAL_TEST_XML.as_bytes());
    let harness = Harness {
        raw_bep: Some(vec![
            test_result_full_line(
                "//a:t",
                1,
                1,
                1,
                "FAILED",
                None,
                Some(false),
                None,
                &[(String::from("test.xml"), first_uri)],
            ),
            test_result_full_line(
                "//a:t",
                1,
                1,
                2,
                "PASSED",
                None,
                Some(false),
                None,
                &[(String::from("test.xml"), retry_uri)],
            ),
            test_flaky_summary_line("//a:t"),
        ]),
        ..harness
    };
    let (code, out, _) = harness.run(&["test", "--output=json"]);
    assert_eq!(code, 0, "{out}");
    let events = json_events(&out);
    let outcomes = events_of_kind(&events, "test_outcome");
    assert_eq!(outcomes.len(), 2, "{out}");
    assert_eq!(outcomes[0]["outcome"], serde_json::json!("failed"));
    assert_eq!(outcomes[0]["attempt"], serde_json::json!(1));
    assert_eq!(
        outcomes[1]["outcome"],
        serde_json::json!("passed_after_retry")
    );
    assert_eq!(outcomes[1]["attempt"], serde_json::json!(2));
    assert_eq!(
        event(&events, "command_finished")["tests"],
        serde_json::json!({"passed": 1, "failed": 0, "cached": 0, "no_cases": 0, "skipped": 0})
    );
}

#[test]
fn test_flaky_success_is_visible_in_text_summary() {
    let harness = Harness::new("test-flaky-text");
    let first_uri = write_bep_artifact(&harness, "first.xml", MINIMAL_TEST_XML.as_bytes());
    let retry_uri = write_bep_artifact(&harness, "retry.xml", MINIMAL_TEST_XML.as_bytes());
    let harness = Harness {
        raw_bep: Some(vec![
            test_result_full_line(
                "//a:t",
                1,
                1,
                1,
                "FAILED",
                None,
                Some(false),
                None,
                &[(String::from("test.xml"), first_uri)],
            ),
            test_result_full_line(
                "//a:t",
                1,
                1,
                2,
                "PASSED",
                None,
                Some(false),
                None,
                &[(String::from("test.xml"), retry_uri)],
            ),
            test_flaky_summary_line("//a:t"),
        ]),
        ..harness
    };
    let (code, _, err) = harness.run(&["test", "--output=text"]);
    assert_eq!(code, 0, "{err}");
    assert!(
        err.contains("dx: flaky success (passed after retry): //a:t (attempt 2)"),
        "{err}"
    );
}

#[test]
fn test_clean_pass_prints_no_flaky_summary() {
    let harness = Harness::new("test-clean-text");
    let uri = write_bep_artifact(&harness, "ok.xml", MINIMAL_TEST_XML.as_bytes());
    let harness = Harness {
        raw_bep: Some(vec![test_result_line(
            "//a:t",
            &[(String::from("test.xml"), uri)],
        )]),
        ..harness
    };
    let (code, _, err) = harness.run(&["test", "--output=text"]);
    assert_eq!(code, 0, "{err}");
    assert!(!err.contains("flaky success"), "{err}");
}

#[test]
fn flaky_summary_without_result_prints_no_attempt() {
    let harness = Harness {
        raw_bep: Some(vec![test_flaky_summary_line("//a:t")]),
        ..Harness::new("test-flaky-summary-only")
    };
    let (_, _, err) = harness.run(&["test", "--output=text"]);
    assert!(
        err.contains("dx: flaky success (passed after retry): //a:t\n"),
        "{err}"
    );
    assert!(!err.contains("(attempt"), "{err}");
}

#[test]
fn later_planned_report_event_is_incomplete_after_an_earlier_write_failure() {
    let harness = Harness::new("test-writefail-later");
    let uri = write_bep_artifact(&harness, "ok.xml", MINIMAL_TEST_XML.as_bytes());
    let harness = Harness {
        raw_bep: Some(vec![test_result_line(
            "//a:t",
            &[(String::from("test.xml"), uri)],
        )]),
        ..harness
    };
    let (code, out, _) = harness.run(&[
        "test",
        "--output=json",
        "--report=results=missing-dir/r.json",
        "--report=junit=j.xml",
    ]);
    assert_eq!(code, 1, "{out}");
    let events = json_events(&out);
    let reports = events_of_kind(&events, "report");
    assert_eq!(reports.len(), 1, "{out}");
    assert_eq!(reports[0]["format"], serde_json::json!("junit"), "{out}");
    assert_eq!(
        reports[0]["results_complete"],
        serde_json::json!(false),
        "{out}"
    );
}

fn evidence(problems: &[&str]) -> super::Evidence {
    let mut evidence = super::Evidence::default();
    for problem in problems {
        evidence.record((*problem).to_owned());
    }
    evidence
}

#[test]
fn evidence_gap_names_what_made_results_incomplete() {
    assert_eq!(
        super::evidence_gap(true, &evidence(&[]), "none reported"),
        Some("none reported".to_owned())
    );
    assert_eq!(
        super::evidence_gap(true, &evidence(&["a", "b"]), "none reported"),
        Some("a".to_owned())
    );
    assert_eq!(
        super::evidence_gap(false, &evidence(&["a", "b"]), "none reported"),
        Some("2 missing or invalid results: a".to_owned())
    );
    assert_eq!(
        super::evidence_gap(false, &evidence(&[]), "none reported"),
        None
    );
}

fn located(xml: &str) -> Vec<(Option<String>, Option<u64>)> {
    let cases = dx_plan::reports::parse_test_xml(xml.as_bytes(), 0, 0).expect("cases");
    cases
        .iter()
        .map(|case| {
            let found = super::test_case(case);
            (found.file, found.line)
        })
        .collect()
}

#[test]
fn a_case_takes_file_and_line_from_the_report_attributes() {
    let xml = r#"<testsuites><testsuite name="s"><testcase name="a" file="t/a_test.py" line="7"><failure message="x:9: no">trace</failure></testcase></testsuite></testsuites>"#;
    assert_eq!(located(xml), [(Some("t/a_test.py".to_owned()), Some(7))]);
}

#[test]
fn a_case_takes_file_and_line_from_the_failure_message() {
    let xml = r#"<testsuites><testsuite name="s"><testcase name="a"><failure message="foo_test.go:42: got 1">t</failure></testcase><testcase name="b"><error>at Foo.method(Foo.java:17)</error></testcase></testsuite></testsuites>"#;
    assert_eq!(
        located(xml),
        [
            (Some("foo_test.go".to_owned()), Some(42)),
            (Some("Foo.java".to_owned()), Some(17)),
        ]
    );
}

#[test]
fn a_case_without_a_location_leaves_file_and_line_unset() {
    let xml = r#"<testsuites><testsuite name="s"><testcase name="a"><failure message="took 12:30:45 at /abs/x.rs:3">t</failure></testcase><testcase name="b"/></testsuite></testsuites>"#;
    assert_eq!(located(xml), [(None, None), (None, None)]);
}

#[test]
fn parse_cases_stamps_run_and_configuration() {
    let result = dx_bep::TestResultRecord {
        label: "//a:t".to_owned(),
        configuration: Some("cfg".to_owned()),
        run: 2,
        shard: 3,
        attempt: 1,
        status: Some("PASSED".to_owned()),
        cached_locally: None,
        duration_millis: None,
        strategy: None,
        files: Vec::new(),
    };
    let cases = super::parse_cases(&result, MINIMAL_TEST_XML.as_bytes()).expect("cases");
    assert!(!cases.is_empty());
    for case in &cases {
        assert_eq!(case.run, 1);
        assert_eq!(case.configuration.as_deref(), Some("cfg"));
    }
    assert!(super::parse_cases(&result, b"no elements").is_err());
}

#[test]
fn coverage_summary_prints_the_total_and_the_worst_files() {
    let mut account = dx_lcov::CellAccount {
        eligible_lines: 20,
        covered_lines: 13,
        ..dx_lcov::CellAccount::default()
    };
    for (path, counts) in [("a.rs", (10, 10)), ("b.rs", (3, 6)), ("c.rs", (0, 4))] {
        account.file_lines.insert(path.to_owned(), counts);
    }
    let text = dx_output::OutputMode::Text { quiet: false };
    let mut err = Vec::new();
    super::coverage_summary(&account, true, text, &mut err);
    let got = String::from_utf8(err).unwrap();
    assert!(got.contains("dx: coverage 65.0% (13/20 lines)"), "{got}");
    assert!(!got.contains("a.rs"), "{got}");
    assert!(
        got.find("c.rs").unwrap() < got.find("b.rs").unwrap(),
        "{got}"
    );
    let mut err = Vec::new();
    super::coverage_summary(&account, false, text, &mut err);
    assert!(!String::from_utf8(err).unwrap().contains("dx: coverage 65"));
    let mut err = Vec::new();
    super::coverage_summary(&account, true, dx_output::OutputMode::Json, &mut err);
    assert!(err.is_empty());
}

#[test]
fn coverage_threshold_passes_without_minimum_and_fails_on_incomplete_evidence() {
    let mut out = Vec::new();
    let mut err = Vec::new();
    let sources = super::Sources::default();
    let text = dx_output::OutputMode::Text { quiet: false };
    assert_eq!(
        super::coverage_threshold(None, None, &sources, text, &mut out, &mut err),
        Ok(true)
    );
    assert_eq!(
        super::coverage_threshold(Some(80), None, &sources, text, &mut out, &mut err),
        Ok(false)
    );
    assert!(out.is_empty() && err.is_empty());
}

#[test]
fn test_timeout_and_failure_outcomes_stay_distinct() {
    let harness = Harness::new("test-outcome-statuses");
    let timeout_uri = write_bep_artifact(&harness, "timeout.xml", MINIMAL_TEST_XML.as_bytes());
    let failed_uri = write_bep_artifact(&harness, "failed.xml", MINIMAL_TEST_XML.as_bytes());
    let harness = Harness {
        raw_bep: Some(vec![
            test_result_full_line(
                "//a:slow",
                1,
                1,
                1,
                "TIMEOUT",
                None,
                None,
                None,
                &[(String::from("test.xml"), timeout_uri)],
            ),
            test_result_full_line(
                "//a:bad",
                1,
                1,
                1,
                "FAILED",
                None,
                None,
                None,
                &[(String::from("test.xml"), failed_uri)],
            ),
        ]),
        ..harness
    };
    let (code, out, _) = harness.run(&["test", "--output=json"]);
    assert_eq!(code, 0, "{out}");
    let events = json_events(&out);
    let outcomes = events_of_kind(&events, "test_outcome");
    assert_eq!(outcomes.len(), 2, "{out}");
    assert_eq!(outcomes[0]["outcome"], serde_json::json!("failed"));
    assert_eq!(outcomes[1]["outcome"], serde_json::json!("timeout"));
    assert_eq!(outcomes[1]["bazel_status"], serde_json::json!("TIMEOUT"));
}

#[test]
fn test_missing_status_reports_unknown_outcome() {
    let harness = Harness::new("test-outcome-unknown");
    let uri = write_bep_artifact(&harness, "ok.xml", MINIMAL_TEST_XML.as_bytes());
    let line = serde_json::json!({
        "id": {"testResult": {"label": "//a:t", "run": 1, "shard": 1, "attempt": 1}},
        "testResult": {"testActionOutput": [{"name": "test.xml", "uri": uri}]},
    })
    .to_string();
    let harness = Harness {
        raw_bep: Some(vec![line]),
        ..harness
    };
    let (code, out, _) = harness.run(&["test", "--output=json"]);
    assert_eq!(code, 0, "{out}");
    let events = json_events(&out);
    let outcomes = events_of_kind(&events, "test_outcome");
    assert_eq!(outcomes.len(), 1, "{out}");
    assert_eq!(outcomes[0]["outcome"], serde_json::json!("unknown"));
    assert!(outcomes[0].get("bazel_status").is_none(), "{out}");
    assert!(outcomes[0].get("evidence_complete").is_none(), "{out}");
}

#[test]
fn test_zero_case_xml_reports_zero_counts() {
    let harness = Harness::new("test-outcome-zero");
    let uri = write_bep_artifact(&harness, "empty.xml", EMPTY_TEST_XML.as_bytes());
    let harness = Harness {
        raw_bep: Some(vec![test_result_line(
            "//a:t",
            &[(String::from("test.xml"), uri)],
        )]),
        ..harness
    };
    let (code, out, _) = harness.run(&["test", "--output=json", "--report=results=res.json"]);
    assert_eq!(code, 0, "{out}");
    let events = json_events(&out);
    assert!(events_of_kind(&events, "test_outcome").is_empty(), "{out}");
    let results: serde_json::Value = serde_json::from_slice(
        &std::fs::read(harness.workspace.join("res.json")).expect("results report"),
    )
    .expect("results json");
    let entry = &results["results"][0];
    assert_eq!(entry["outcome"], serde_json::json!("no_cases"), "{results}");
    assert_eq!(entry["cases_total"], serde_json::json!(0), "{results}");
    assert_eq!(
        event(&events, "command_finished")["tests"],
        serde_json::json!({"passed": 1, "failed": 0, "cached": 0, "no_cases": 1, "skipped": 0})
    );
}

#[test]
fn test_cached_no_cases_result_counts_cached_and_no_cases() {
    let harness = Harness::new("test-outcome-cached-zero");
    let uri = write_bep_artifact(
        &harness,
        "empty.xml",
        "<testsuites></testsuites>".as_bytes(),
    );
    let harness = Harness {
        raw_bep: Some(vec![test_result_full_line(
            "//a:t",
            1,
            1,
            1,
            "PASSED",
            None,
            Some(true),
            None,
            &[(String::from("test.xml"), uri)],
        )]),
        ..harness
    };
    let (code, out, _) = harness.run(&["test", "--output=json"]);
    assert_eq!(code, 0, "{out}");
    let events = json_events(&out);
    assert!(events_of_kind(&events, "test_outcome").is_empty(), "{out}");
    assert_eq!(
        event(&events, "command_finished")["tests"],
        serde_json::json!({"passed": 1, "failed": 0, "cached": 1, "no_cases": 1, "skipped": 0})
    );
}

#[test]
fn test_status_only_result_is_incomplete_evidence() {
    let harness = Harness {
        raw_bep: Some(vec![String::from(
            "{\"id\": {\"testResult\": {\"label\": \"//a:t\"}}, \"testResult\": {\"status\": \"PASSED\"}}",
        )]),
        ..Harness::new("test-outcome-nofile")
    };
    let (code, out, _) = harness.run(&["test", "--output=json"]);
    assert_eq!(code, 1, "{out}");
    let events = json_events(&out);
    let outcomes = events_of_kind(&events, "test_outcome");
    assert_eq!(outcomes.len(), 1, "{out}");
    assert_eq!(outcomes[0]["outcome"], serde_json::json!("passed"));
    assert_eq!(outcomes[0]["evidence_complete"], serde_json::json!(false));
    assert_eq!(outcomes[0]["artifacts"]["missing"], serde_json::json!(1));
    let finished = event(&events, "command_finished");
    assert_eq!(finished["results_complete"], serde_json::json!(false));
}

#[test]
fn test_single_missing_xml_fails_when_bazel_passed() {
    let harness = Harness::new("test-partial-missing");
    let raw = (0..4)
        .map(|index| {
            test_result_line(
                &format!("//a:ok{index}"),
                &[(
                    String::from("test.xml"),
                    write_bep_artifact(
                        &harness,
                        &format!("strict-ok-{index}.xml"),
                        MINIMAL_TEST_XML.as_bytes(),
                    ),
                )],
            )
        })
        .collect::<Vec<_>>();
    let mut lines = raw;
    lines.push(test_result_line(
        "//a:missing",
        &[(String::from("test.xml"), missing_uri("strict-missing.xml"))],
    ));
    let harness = Harness {
        raw_bep: Some(lines),
        ..harness
    };
    let (code, out, _) = harness.run(&["test", "--output=json"]);
    assert_eq!(code, 1, "{out}");
    let events = json_events(&out);
    let outcomes = events_of_kind(&events, "test_outcome");
    assert_eq!(outcomes.len(), 1, "{out}");
    assert_eq!(
        event(&events, "command_finished")["tests"],
        serde_json::json!({"passed": 4, "failed": 0, "cached": 0, "no_cases": 0, "skipped": 0})
    );
    let missing = outcomes
        .iter()
        .find(|outcome| outcome["target"] == serde_json::json!("//a:missing"))
        .expect("missing target outcome");
    assert_eq!(missing["evidence_complete"], serde_json::json!(false));
    assert_eq!(missing["artifacts"]["missing"], serde_json::json!(1));
    let incomplete: Vec<_> = events_of_kind(&events, "error")
        .into_iter()
        .filter(|error| error["code"] == serde_json::json!("incomplete_results"))
        .collect();
    assert_eq!(incomplete.len(), 1, "{out}");
    let finished = event(&events, "command_finished");
    assert_eq!(finished["exit_code"], serde_json::json!(1));
    assert_eq!(finished["results_complete"], serde_json::json!(false));
}

#[test]
fn test_missing_xml_reports_text_failure() {
    let harness = Harness::new("test-strict-text");
    let raw = (0..4)
        .map(|index| {
            test_result_line(
                &format!("//a:ok{index}"),
                &[(
                    String::from("test.xml"),
                    write_bep_artifact(
                        &harness,
                        &format!("strict-text-ok-{index}.xml"),
                        MINIMAL_TEST_XML.as_bytes(),
                    ),
                )],
            )
        })
        .collect::<Vec<_>>();
    let mut lines = raw;
    lines.push(test_result_line(
        "//a:missing",
        &[(
            String::from("test.xml"),
            missing_uri("strict-text-missing.xml"),
        )],
    ));
    let harness = Harness {
        raw_bep: Some(lines),
        ..harness
    };
    let (code, _, err) = harness.run(&["test", "--output=text"]);
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("incomplete_results"), "{err}");
}

#[test]
fn test_complete_run_passes() {
    let harness = Harness::new("test-strict-complete");
    let uri = write_bep_artifact(&harness, "ok.xml", MINIMAL_TEST_XML.as_bytes());
    let harness = Harness {
        raw_bep: Some(vec![test_result_line(
            "//a:t",
            &[(String::from("test.xml"), uri)],
        )]),
        ..harness
    };
    let (code, _, err) = harness.run(&["test", "--output=text", "--report=junit=out.xml"]);
    assert_eq!(code, 0, "{err}");
    assert!(!err.contains("incomplete_results"), "{err}");
    let document = std::fs::read(harness.workspace.join("out.xml")).expect("junit");
    let text = String::from_utf8(document).expect("utf8");
    assert!(text.contains("<testsuite name=\"//a:t\""), "{text}");
}

#[test]
fn test_failed_build_preserves_code_and_reports_outcomes() {
    let harness = Harness::new("test-outcome-bazelfail");
    let uri = write_bep_artifact(&harness, "ok.xml", MINIMAL_TEST_XML.as_bytes());
    let harness = Harness {
        bazel_code: 4,
        raw_bep: Some(vec![test_result_line(
            "//a:t",
            &[(String::from("test.xml"), uri)],
        )]),
        ..harness
    };
    let (code, out, _) = harness.run(&["test", "--output=json"]);
    assert_eq!(code, 4, "{out}");
    let events = json_events(&out);
    assert!(events_of_kind(&events, "test_outcome").is_empty(), "{out}");
    assert_eq!(
        event(&events, "command_finished")["tests"],
        serde_json::json!({"passed": 1, "failed": 0, "cached": 0, "no_cases": 0, "skipped": 0})
    );
    let failed: Vec<_> = events_of_kind(&events, "error")
        .into_iter()
        .filter(|error| error["code"] == serde_json::json!("bazel_failed"))
        .collect();
    assert_eq!(failed.len(), 1, "{out}");
    assert_eq!(failed[0]["cause"], serde_json::json!("bazel"), "{out}");
    assert_eq!(failed[0]["status"], serde_json::json!(4), "{out}");
    assert_eq!(failed[0]["phase"], serde_json::json!("execute"), "{out}");
    let finished = event(&events, "command_finished");
    assert_eq!(finished["exit_code"], serde_json::json!(4));
    assert_eq!(finished["results_complete"], serde_json::json!(false));
}

#[test]
fn test_label_pattern_without_tests_reports_no_tests() {
    let harness = Harness {
        bazel_code: 4,
        ..Harness::new("test-no-tests-pattern")
    };
    let (code, out, err) = harness.run(&["test", "--output=json"]);
    assert_eq!(code, 4, "{out}{err}");
    let events = json_events(&out);
    assert!(
        errors_with_code(&events, "bazel_failed").is_empty(),
        "{out}"
    );
    let found = errors_with_code(&events, "no_tests");
    assert_eq!(found.len(), 1, "{out}");
    assert_eq!(found[0]["status"], serde_json::json!(4), "{out}");
    assert!(
        err.contains("next: check the target or path scope and run the command again."),
        "{err}"
    );
    assert_eq!(
        event(&events, "command_finished")["exit_code"],
        serde_json::json!(4)
    );
}

fn failing_run(name: &str, statuses: &[&str]) -> (i32, String, String) {
    let harness = Harness::new(name);
    let lines = statuses
        .iter()
        .enumerate()
        .map(|(index, status)| {
            let uri = write_bep_artifact(
                &harness,
                &format!("{index}.xml"),
                MINIMAL_TEST_XML.as_bytes(),
            );
            test_result_full_line(
                &format!("//a:t{index}"),
                1,
                1,
                1,
                status,
                None,
                None,
                None,
                &[(String::from("test.xml"), uri)],
            )
        })
        .collect();
    let harness = Harness {
        bazel_code: 3,
        raw_bep: Some(lines),
        ..harness
    };
    harness.run(&["test", "--output=json"])
}

fn errors_with_code<'a>(events: &'a [serde_json::Value], code: &str) -> Vec<&'a serde_json::Value> {
    events_of_kind(events, "error")
        .into_iter()
        .filter(|error| error["code"] == serde_json::json!(code))
        .collect()
}

#[test]
fn failing_tests_emit_tests_failed_with_counts() {
    let (code, out, err) = failing_run("test-tests-failed", &["PASSED", "FAILED"]);
    assert_eq!(code, 3, "{out}{err}");
    let events = json_events(&out);
    let errors = events_of_kind(&events, "error");
    assert_eq!(errors.len(), 1, "{out}");
    assert_eq!(
        errors[0]["code"],
        serde_json::json!("tests_failed"),
        "{out}"
    );
    assert_eq!(errors[0]["cause"], serde_json::json!("test"), "{out}");
    assert_eq!(errors[0]["status"], serde_json::json!(3), "{out}");
    assert_eq!(errors[0]["phase"], serde_json::json!("execute"), "{out}");
    assert_eq!(
        errors[0]["message"],
        serde_json::json!("1 test failed; read the test_outcome events"),
        "{out}"
    );
    assert!(
        errors_with_code(&events, "bazel_failed").is_empty(),
        "{out}"
    );
    assert!(!out.contains("dx status"), "{out}");
    assert!(err.is_empty(), "{err}");
    let finished = event(&events, "command_finished");
    assert_eq!(
        finished["tests"],
        serde_json::json!({"passed": 1, "failed": 1, "cached": 0, "no_cases": 0, "skipped": 0})
    );
    assert_eq!(finished["exit_code"], serde_json::json!(3));
}

fn attempts_run(
    name: &str,
    attempts: &[(&str, u32, &str)],
    flaky: &[&str],
) -> (i32, String, String) {
    let harness = Harness::new(name);
    let mut lines: Vec<String> = attempts
        .iter()
        .enumerate()
        .map(|(index, (label, attempt, status))| {
            let uri = write_bep_artifact(
                &harness,
                &format!("{index}.xml"),
                MINIMAL_TEST_XML.as_bytes(),
            );
            test_result_full_line(
                label,
                1,
                1,
                *attempt,
                status,
                None,
                Some(false),
                None,
                &[(String::from("test.xml"), uri)],
            )
        })
        .collect();
    lines.extend(flaky.iter().map(|label| test_flaky_summary_line(label)));
    let harness = Harness {
        bazel_code: 3,
        raw_bep: Some(lines),
        ..harness
    };
    harness.run(&["test", "--output=json"])
}

#[test]
fn flaky_target_with_real_failure_counts_one_failure() {
    let (code, out, err) = attempts_run(
        "test-flaky-and-failed",
        &[
            ("//a:t", 1, "FAILED"),
            ("//a:t", 2, "PASSED"),
            ("//b:t", 1, "FAILED"),
        ],
        &["//a:t"],
    );
    assert_eq!(code, 3, "{out}{err}");
    let events = json_events(&out);
    let errors = errors_with_code(&events, "tests_failed");
    assert_eq!(errors.len(), 1, "{out}");
    assert_eq!(
        errors[0]["message"],
        serde_json::json!("1 test failed; read the test_outcome events"),
        "{out}"
    );
    assert_eq!(
        event(&events, "command_finished")["tests"],
        serde_json::json!({"passed": 1, "failed": 1, "cached": 0, "no_cases": 0, "skipped": 0})
    );
    let outcomes = events_of_kind(&events, "test_outcome");
    assert_eq!(outcomes.len(), 3, "{out}");
    let finals: Vec<(String, u64, Option<serde_json::Value>)> = outcomes
        .iter()
        .map(|outcome| {
            (
                outcome["target"].as_str().expect("target").to_owned(),
                outcome["attempt"].as_u64().expect("attempt"),
                outcome.get("final").cloned(),
            )
        })
        .collect();
    assert!(
        finals.contains(&("//a:t".to_owned(), 1, Some(serde_json::json!(false)))),
        "{out}"
    );
    assert!(finals.contains(&("//a:t".to_owned(), 2, None)), "{out}");
    assert!(finals.contains(&("//b:t".to_owned(), 1, None)), "{out}");
    assert_eq!(final_failures(&outcomes), 1, "{out}");
    assert!(err.is_empty(), "{err}");
}

fn final_failures(outcomes: &[&serde_json::Value]) -> usize {
    outcomes
        .iter()
        .filter(|outcome| outcome["outcome"] == "failed" && outcome.get("final").is_none())
        .count()
}

#[test]
fn retried_failure_counts_once() {
    let (code, out, err) = attempts_run(
        "test-retried-failure",
        &[
            ("//a:t", 1, "FAILED"),
            ("//a:t", 2, "FAILED"),
            ("//a:t", 3, "FAILED"),
        ],
        &[],
    );
    assert_eq!(code, 3, "{out}{err}");
    let events = json_events(&out);
    let errors = errors_with_code(&events, "tests_failed");
    assert_eq!(errors.len(), 1, "{out}");
    assert_eq!(
        errors[0]["message"],
        serde_json::json!("1 test failed; read the test_outcome events"),
        "{out}"
    );
    assert_eq!(
        event(&events, "command_finished")["tests"],
        serde_json::json!({"passed": 0, "failed": 1, "cached": 0, "no_cases": 0, "skipped": 0})
    );
    let outcomes = events_of_kind(&events, "test_outcome");
    assert_eq!(outcomes.len(), 3, "{out}");
    for outcome in &outcomes {
        let expected = if outcome["attempt"] == 3 {
            None
        } else {
            Some(&serde_json::json!(false))
        };
        assert_eq!(outcome.get("final"), expected, "{out}");
    }
    assert_eq!(final_failures(&outcomes), 1, "{out}");
}

#[test]
fn every_failed_test_still_reports_counts() {
    let (code, out, _) = failing_run("test-all-failed", &["FAILED", "TIMEOUT"]);
    assert_eq!(code, 3, "{out}");
    let events = json_events(&out);
    let failed = errors_with_code(&events, "tests_failed");
    assert_eq!(failed.len(), 1, "{out}");
    assert_eq!(
        failed[0]["message"],
        serde_json::json!("2 tests failed; read the test_outcome events"),
        "{out}"
    );
    assert_eq!(
        event(&events, "command_finished")["tests"],
        serde_json::json!({"passed": 0, "failed": 2, "cached": 0, "no_cases": 0, "skipped": 0})
    );
}

#[test]
fn exit_three_without_failed_results_keeps_bazel_failed() {
    let (code, out, _) = failing_run("test-exit-three-passed", &["PASSED"]);
    assert_eq!(code, 3, "{out}");
    let events = json_events(&out);
    assert!(
        errors_with_code(&events, "tests_failed").is_empty(),
        "{out}"
    );
    let failed = errors_with_code(&events, "bazel_failed");
    assert_eq!(failed.len(), 1, "{out}");
    assert_eq!(failed[0]["cause"], serde_json::json!("bazel"), "{out}");
    assert_eq!(failed[0]["status"], serde_json::json!(3), "{out}");
    assert_eq!(
        event(&events, "command_finished")["tests"],
        serde_json::json!({"passed": 1, "failed": 0, "cached": 0, "no_cases": 0, "skipped": 0})
    );
}

#[test]
fn test_server_crash_reports_server_crashed() {
    let harness = Harness::new("test-server-crash");
    let uri = write_bep_artifact(&harness, "0.xml", MINIMAL_TEST_XML.as_bytes());
    let line = test_result_full_line(
        "//a:t0",
        1,
        1,
        1,
        "PASSED",
        None,
        None,
        None,
        &[(String::from("test.xml"), uri)],
    );
    let harness = Harness {
        bazel_code: 37,
        raw_bep: Some(vec![line]),
        ..harness
    };
    let (code, out, _) = harness.run(&["test", "--output=json", "//a:t0"]);
    assert_eq!(code, 37, "{out}");
    let events = json_events(&out);
    let crashed = errors_with_code(&events, "server_crashed");
    assert_eq!(crashed.len(), 1, "{out}");
    assert_eq!(crashed[0]["cause"], serde_json::json!("host"), "{out}");
    assert!(
        errors_with_code(&events, "bazel_failed").is_empty(),
        "{out}"
    );
}

#[test]
fn test_signalled_run_emits_no_test_outcome() {
    let mut harness = Harness::new("test-outcome-signalled");
    harness.signalled = true;
    let (code, out, err) = harness.run(&["test", "--output=json"]);
    assert_ne!(code, 0, "{out}{err}");
    assert!(!out.contains("test_outcome"), "{out}");
}

#[test]
fn test_coverage_outcomes_mark_only_final_attempt_evidence() {
    let harness = Harness::new("cov-outcome-retry");
    let final_uri = write_bep_artifact(&harness, "final.dat", MINIMAL_LCOV.as_bytes());
    let harness = Harness {
        raw_bep: Some(vec![
            test_result_identity_line(
                "//a:t",
                1,
                1,
                1,
                &[(String::from("test.lcov"), missing_uri("cov-attempt1.dat"))],
            ),
            test_result_identity_line("//a:t", 1, 1, 2, &[(String::from("test.lcov"), final_uri)]),
        ]),
        ..harness
    };
    let (code, out, _) = harness.run(&["coverage", "--output=json", "--report=results=res.json"]);
    assert_eq!(code, 0, "{out}");
    let events = json_events(&out);
    assert!(events_of_kind(&events, "test_outcome").is_empty(), "{out}");
    assert_eq!(
        event(&events, "command_finished")["tests"],
        serde_json::json!({"passed": 1, "failed": 0, "cached": 0, "no_cases": 0, "skipped": 0})
    );
    let results: serde_json::Value = serde_json::from_slice(
        &std::fs::read(harness.workspace.join("res.json")).expect("results report"),
    )
    .expect("results json");
    let entries = results["results"].as_array().expect("results");
    assert_eq!(entries.len(), 1, "{results}");
    assert_eq!(entries[0]["attempt"], serde_json::json!(2));
    assert!(entries[0].get("artifacts").is_none(), "{results}");
    assert!(entries[0].get("final").is_none(), "{results}");
}

const MIXED_CASES_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?><testsuites><testsuite name="s"><testcase name="passes" classname="c"/><testcase name="fails" classname="c"><failure message="expected 1">trace</failure></testcase><testcase name="errs" classname="c"><error>panic text</error></testcase><testcase name="ignored" classname="c"><skipped message="not on linux"/></testcase></testsuite></testsuites>"#;

fn completed_line(label: &str, children: &[serde_json::Value]) -> String {
    serde_json::json!({
        "id": {"targetCompleted": {"label": label}},
        "completed": {"success": true},
        "children": children,
    })
    .to_string()
}

fn summary_line(label: &str, status: &str) -> String {
    serde_json::json!({
        "id": {"testSummary": {"label": label}},
        "testSummary": {"overallStatus": status},
    })
    .to_string()
}

#[test]
fn a_result_without_xml_beside_one_with_xml_is_incomplete() {
    let harness = Harness::new("test-mixed-xml");
    let uri = write_bep_artifact(&harness, "ok.xml", MINIMAL_TEST_XML.as_bytes());
    let harness = Harness {
        raw_bep: Some(vec![
            test_result_line("//a:ok", &[(String::from("test.xml"), uri)]),
            test_result_line("//b:bare", &[]),
        ]),
        ..harness
    };
    let (code, out, err) = harness.run(&["test", "--output=text", "--report=junit=out.xml"]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(
            err.contains(
                "dx: incomplete_results: test.xml for //b:bare (run 1, shard 1, attempt 1) was not reported"
            ),
            "{err}"
        );
    let document = std::fs::read_to_string(harness.workspace.join("out.xml")).expect("junit");
    assert!(
        document.contains("<testsuite name=\"//a:ok\""),
        "{document}"
    );
    assert!(
        document.contains("test.xml for //b:bare (run 1, shard 1, attempt 1) was not reported"),
        "{document}"
    );
    let (code, out, _) = harness.run(&["test", "--output=json", "--report=junit=out.xml"]);
    assert_eq!(code, 1, "{out}");
    let events = json_events(&out);
    let outcomes = events_of_kind(&events, "test_outcome");
    assert_eq!(outcomes.len(), 1, "{out}");
    assert_eq!(outcomes[0]["target"], serde_json::json!("//b:bare"));
    assert_eq!(outcomes[0]["evidence_complete"], serde_json::json!(false));
    assert_eq!(outcomes[0]["artifacts"]["missing"], serde_json::json!(1));
    assert_eq!(
        event(&events, "report")["results_complete"],
        serde_json::json!(false)
    );
    let finished = event(&events, "command_finished");
    assert_eq!(finished["exit_code"], serde_json::json!(1));
    assert_eq!(finished["results_complete"], serde_json::json!(false));
    let incomplete: Vec<_> = events_of_kind(&events, "error")
        .into_iter()
        .filter(|error| error["code"] == "incomplete_results")
        .collect();
    assert_eq!(incomplete.len(), 1, "{out}");
}

#[test]
fn readable_reports_emit_each_case_with_its_identity() {
    let harness = Harness::new("test-cases");
    let uri = write_bep_artifact(&harness, "cases.xml", MIXED_CASES_XML.as_bytes());
    let harness = Harness {
        bazel_code: 3,
        raw_bep: Some(vec![test_result_full_line(
            "//a:t",
            1,
            2,
            1,
            "FAILED",
            Some("cfg1"),
            Some(true),
            Some(40),
            &[(String::from("test.xml"), uri)],
        )]),
        ..harness
    };
    let (code, out, _) = harness.run(&["test", "--output=json", "--report=results=res.json"]);
    assert_eq!(code, 3, "{out}");
    let events = json_events(&out);
    let outcome = event(&events, "test_outcome");
    assert_eq!(outcome["target"], serde_json::json!("//a:t"));
    assert_eq!(outcome["configuration"], serde_json::json!("cfg1"));
    assert_eq!(outcome["shard"], serde_json::json!(2));
    assert_eq!(outcome["cached"], serde_json::json!(true));
    assert_eq!(outcome["cases_total"], serde_json::json!(4));
    assert_eq!(outcome["cases_failed"], serde_json::json!(1));
    assert_eq!(outcome["cases_error"], serde_json::json!(1));
    assert_eq!(outcome["cases_skipped"], serde_json::json!(1));
    assert_eq!(
        outcome["failing_cases"],
        serde_json::json!([
            {"classname": "c", "name": "fails", "outcome": "failed", "message": "expected 1"},
            {"classname": "c", "name": "errs", "outcome": "error", "message": "panic text"},
        ])
    );
    let document: serde_json::Value = serde_json::from_slice(
        &std::fs::read(harness.workspace.join("res.json")).expect("results report"),
    )
    .expect("results json");
    let entries = document["results"].as_array().expect("results");
    assert_eq!(entries.len(), 1, "{document}");
    let named: Vec<(&str, &str)> = entries[0]["cases"]
        .as_array()
        .expect("cases")
        .iter()
        .map(|case| {
            (
                case["name"].as_str().expect("name"),
                case["outcome"].as_str().expect("outcome"),
            )
        })
        .collect();
    assert_eq!(
        named,
        vec![
            ("passes", "passed"),
            ("fails", "failed"),
            ("errs", "error"),
            ("ignored", "skipped"),
        ]
    );
    let reports = events_of_kind(&events, "report");
    assert_eq!(reports[0]["format"], serde_json::json!("results"));
    assert_eq!(reports[0]["path"], serde_json::json!("res.json"));
}

fn failing_result_files(harness: &Harness, with_xml: bool) -> Vec<(String, String)> {
    let mut files = vec![(
        String::from("test.lcov"),
        write_bep_artifact(harness, "coverage.dat", MINIMAL_LCOV.as_bytes()),
    )];
    if with_xml {
        files.push((
            String::from("test.xml"),
            write_bep_artifact(harness, "cases.xml", MIXED_CASES_XML.as_bytes()),
        ));
    }
    files.push((
        String::from("test.log"),
        write_bep_artifact(harness, "test.log", b"first\nboom happened\n"),
    ));
    files.push((
        String::from("test.outputs__outputs.zip"),
        write_bep_artifact(harness, "outputs.zip", b"zip"),
    ));
    files
}

#[test]
fn test_failing_outcome_carries_log_path_and_tail() {
    let harness = Harness::new("test-failing-log");
    let files = failing_result_files(&harness, true);
    let harness = Harness {
        bazel_code: 3,
        raw_bep: Some(vec![test_result_full_line(
            "//a:t", 1, 1, 1, "FAILED", None, None, None, &files,
        )]),
        ..harness
    };
    let (_, out, _) = harness.run(&["test", "--output=json"]);
    let events = json_events(&out);
    let outcome = event(&events, "test_outcome");
    assert!(outcome["log_path"].as_str().is_some(), "{out}");
    assert!(
        outcome["outputs_path"]
            .as_str()
            .expect("outputs")
            .ends_with("outputs.zip"),
        "{out}"
    );
    assert!(
        outcome["log_tail"]
            .as_str()
            .expect("tail")
            .contains("boom happened"),
        "{out}"
    );
}

#[test]
fn test_text_mode_names_failing_cases_and_log() {
    let harness = Harness::new("test-failing-text");
    let files = failing_result_files(&harness, true);
    let harness = Harness {
        bazel_code: 3,
        raw_bep: Some(vec![test_result_full_line(
            "//a:t", 1, 1, 1, "FAILED", None, None, None, &files,
        )]),
        ..harness
    };
    let (_, _, err) = harness.run(&["test", "--output=text"]);
    let line = err
        .lines()
        .find(|line| line.starts_with("dx: failed: //a:t: fails, errs"))
        .unwrap_or_else(|| panic!("{err}"));
    assert!(
        line.contains("(log: ") && line.contains("test.log)"),
        "{line}"
    );
}

#[test]
fn coverage_failing_outcome_carries_failing_cases_and_log() {
    let harness = Harness::new("cov-failing-cases");
    let files = failing_result_files(&harness, true);
    let harness = Harness {
        bazel_code: 3,
        raw_bep: Some(vec![test_result_full_line(
            "//a:t", 1, 1, 1, "FAILED", None, None, None, &files,
        )]),
        ..harness
    };
    let (_, out, _) = harness.run(&["coverage", "--output=json"]);
    let events = json_events(&out);
    let outcome = event(&events, "test_outcome");
    let failing = outcome["failing_cases"].as_array().expect("failing");
    let names: Vec<&str> = failing
        .iter()
        .map(|case| case["name"].as_str().expect("name"))
        .collect();
    assert_eq!(names, vec!["fails", "errs"], "{out}");
    assert!(outcome["log_path"].as_str().is_some(), "{out}");
    assert!(
        outcome["log_tail"]
            .as_str()
            .expect("tail")
            .contains("boom happened"),
        "{out}"
    );
    assert_eq!(
        event(&events, "command_finished")["results_complete"],
        serde_json::json!(true),
        "{out}"
    );
}

#[test]
fn coverage_failing_outcome_without_test_xml_stays_complete() {
    let harness = Harness::new("cov-failing-no-xml");
    let files = failing_result_files(&harness, false);
    let harness = Harness {
        bazel_code: 3,
        raw_bep: Some(vec![test_result_full_line(
            "//a:t", 1, 1, 1, "FAILED", None, None, None, &files,
        )]),
        ..harness
    };
    let (_, out, _) = harness.run(&["coverage", "--output=json"]);
    let events = json_events(&out);
    let outcome = event(&events, "test_outcome");
    assert!(outcome.get("failing_cases").is_none(), "{out}");
    assert_eq!(
        event(&events, "command_finished")["results_complete"],
        serde_json::json!(true),
        "{out}"
    );
}

#[test]
fn passed_count_is_per_run_and_shard() {
    let harness = Harness::new("test-count-unit");
    let mut lines = Vec::new();
    let xml = vec![(
        String::from("test.xml"),
        write_bep_artifact(&harness, "ok.xml", MINIMAL_TEST_XML.as_bytes()),
    )];
    for run in 1..=2 {
        for shard in 1..=2 {
            let attempt_two = run == 1 && shard == 1;
            if attempt_two {
                lines.push(test_result_full_line(
                    "//a:t", run, shard, 1, "FAILED", None, None, None, &xml,
                ));
                lines.push(test_result_full_line(
                    "//a:t", run, shard, 2, "PASSED", None, None, None, &xml,
                ));
            } else {
                lines.push(test_result_identity_line("//a:t", run, shard, 1, &xml));
            }
        }
    }
    let harness = Harness {
        raw_bep: Some(lines),
        ..harness
    };
    let (_, out, _) = harness.run(&["test", "--output=json", "--report=results=res.json"]);
    let events = json_events(&out);
    let tests = &event(&events, "command_finished")["tests"];
    assert_eq!(tests["passed"], serde_json::json!(4), "{out}");
    assert_eq!(tests["failed"], serde_json::json!(0), "{out}");
    let results: serde_json::Value = serde_json::from_slice(
        &std::fs::read(harness.workspace.join("res.json")).expect("results report"),
    )
    .expect("results json");
    let passed = results["results"]
        .as_array()
        .expect("results")
        .iter()
        .filter(|entry| entry["bazel_status"] == "PASSED" && entry["final"] != false)
        .count();
    assert_eq!(passed, 4, "{results}");
}

#[test]
fn failing_case_past_the_case_limit_is_still_reported() {
    let harness = Harness::new("test-late-failure");
    let mut xml = String::from("<testsuites><testsuite name=\"s\">");
    for index in 0..1500 {
        xml.push_str(&format!("<testcase name=\"case{index}\"/>"));
    }
    xml.push_str("<testcase name=\"late\"><failure message=\"late boom\"/></testcase>");
    xml.push_str("</testsuite></testsuites>");
    let uri = write_bep_artifact(&harness, "late.xml", xml.as_bytes());
    let harness = Harness {
        bazel_code: 3,
        raw_bep: Some(vec![test_result_full_line(
            "//a:t",
            1,
            1,
            1,
            "FAILED",
            None,
            None,
            None,
            &[(String::from("test.xml"), uri)],
        )]),
        ..harness
    };
    let (_, out, _) = harness.run(&["test", "--output=json"]);
    let events = json_events(&out);
    let outcome = event(&events, "test_outcome");
    let failing = outcome["failing_cases"].as_array().expect("failing");
    assert_eq!(failing.len(), 1, "{out}");
    assert_eq!(failing[0]["name"], serde_json::json!("late"));
}

#[test]
fn default_results_report_failure_fails_the_run() {
    let harness = Harness::new("test-results-default-blocked");
    let uri = write_bep_artifact(&harness, "ok.xml", MINIMAL_TEST_XML.as_bytes());
    let harness = Harness {
        raw_bep: Some(vec![test_result_line(
            "//a:t",
            &[(String::from("test.xml"), uri)],
        )]),
        ..harness
    };
    std::fs::create_dir_all(harness.workspace.join(".dx/local/results.json")).expect("block");
    let (code, out, _) = harness.run(&["test", "--output=json"]);
    assert_eq!(code, 1, "{out}");
    let events = json_events(&out);
    assert!(
        events.iter().any(|event| event["code"] == "report_failed"),
        "{out}"
    );
    let finished = events.last().expect("command_finished");
    assert_eq!(finished["event"], "command_finished", "{out}");
    assert_eq!(finished["results_complete"], false, "{out}");
}

#[test]
fn default_results_report_failure_keeps_the_bazel_code() {
    let harness = Harness::new("test-results-default-blocked-bazel");
    let uri = write_bep_artifact(&harness, "bad.xml", MIXED_TEST_XML.as_bytes());
    let harness = Harness {
        bazel_code: 3,
        raw_bep: Some(vec![test_result_full_line(
            "//a:t",
            1,
            1,
            1,
            "FAILED",
            None,
            None,
            None,
            &[(String::from("test.xml"), uri)],
        )]),
        ..harness
    };
    std::fs::create_dir_all(harness.workspace.join(".dx/local/results.json")).expect("block");
    let (code, out, _) = harness.run(&["test", "--output=json"]);
    assert_eq!(code, 3, "{out}");
    let events = json_events(&out);
    let finished = events.last().expect("command_finished");
    assert_eq!(finished["results_complete"], false, "{out}");
}

#[test]
fn results_document_run_id_matches_command_finished() {
    let harness = Harness::new("test-results-run-id");
    let uri = write_bep_artifact(&harness, "ok.xml", MINIMAL_TEST_XML.as_bytes());
    let harness = Harness {
        raw_bep: Some(vec![test_result_line(
            "//a:t",
            &[(String::from("test.xml"), uri)],
        )]),
        ..harness
    };
    let (code, out, _) = harness.run(&["test", "--output=json"]);
    assert_eq!(code, 0, "{out}");
    let events = json_events(&out);
    let finished = events.last().expect("command_finished");
    let run_id = finished["run_id"].as_str().expect("run_id in event");
    let file = std::fs::read_to_string(harness.workspace.join(".dx/local/results.json"))
        .expect("results file");
    let document: serde_json::Value = serde_json::from_str(&file).expect("json");
    assert_eq!(document["run_id"], run_id);
    assert_eq!(document["record_version"], 1);
}

#[test]
fn explicit_results_report_carries_the_run_id() {
    let harness = Harness::new("test-results-run-id-explicit");
    let uri = write_bep_artifact(&harness, "ok.xml", MINIMAL_TEST_XML.as_bytes());
    let harness = Harness {
        raw_bep: Some(vec![test_result_line(
            "//a:t",
            &[(String::from("test.xml"), uri)],
        )]),
        ..harness
    };
    let (code, out, _) = harness.run(&[
        "test",
        "--output=json",
        "--report",
        "results=elsewhere.json",
    ]);
    assert_eq!(code, 0, "{out}");
    let events = json_events(&out);
    let finished = events.last().expect("command_finished");
    let run_id = finished["run_id"].as_str().expect("run_id in event");
    let file =
        std::fs::read_to_string(harness.workspace.join("elsewhere.json")).expect("results file");
    let document: serde_json::Value = serde_json::from_str(&file).expect("json");
    assert_eq!(document["run_id"], run_id);
    assert!(!harness.workspace.join(".dx/local/results.json").exists());
}

#[test]
fn case_detail_is_bounded_and_says_so() {
    let harness = Harness::new("test-case-bounds");
    let long = "é".repeat(3000);
    let mut xml = String::from("<testsuites><testsuite name=\"s\">");
    xml.push_str(&format!(
        "<testcase name=\"long\"><failure message=\"{long}\"/></testcase>"
    ));
    for index in 0..1000 {
        xml.push_str(&format!("<testcase name=\"case{index}\"/>"));
    }
    xml.push_str("</testsuite></testsuites>");
    let uri = write_bep_artifact(&harness, "many.xml", xml.as_bytes());
    let harness = Harness {
        bazel_code: 3,
        raw_bep: Some(vec![test_result_full_line(
            "//a:t",
            1,
            1,
            1,
            "FAILED",
            None,
            None,
            None,
            &[(String::from("test.xml"), uri)],
        )]),
        ..harness
    };
    let (_, out, _) = harness.run(&["test", "--output=json", "--report=results=res.json"]);
    let events = json_events(&out);
    let outcome = event(&events, "test_outcome");
    assert_eq!(outcome["cases_total"], serde_json::json!(1001));
    let failing = outcome["failing_cases"].as_array().expect("failing");
    assert_eq!(failing.len(), 1);
    let message = failing[0]["message"].as_str().expect("message");
    assert_eq!(message.len(), dx_output::EVENT_MESSAGE_LIMIT);
    assert!(message.chars().all(|c| c == 'é'));
    assert!(outcome.get("failing_cases_omitted").is_none(), "{out}");
    let document: serde_json::Value = serde_json::from_slice(
        &std::fs::read(harness.workspace.join("res.json")).expect("results report"),
    )
    .expect("results json");
    let cases = document["results"][0]["cases"].as_array().expect("cases");
    assert_eq!(cases.len(), 1000);
    assert_eq!(
        document["results"][0]["cases_omitted"],
        serde_json::json!(1)
    );
    assert_eq!(cases[0]["message"].as_str().expect("message").len(), 4096);
}

#[test]
fn an_announced_result_that_never_arrives_is_incomplete() {
    let harness = Harness::new("test-gap");
    let uri = write_bep_artifact(&harness, "shard1.xml", MINIMAL_TEST_XML.as_bytes());
    let harness = Harness {
        raw_bep: Some(vec![
            completed_line(
                "//a:t",
                &[
                    serde_json::json!({"testResult": {"label": "//a:t", "run": 1, "shard": 1, "attempt": 1}}),
                    serde_json::json!({"testResult": {"label": "//a:t", "run": 1, "shard": 2, "attempt": 1}}),
                ],
            ),
            test_result_line("//a:t", &[(String::from("test.xml"), uri)]),
        ]),
        ..harness
    };
    let (code, out, _) = harness.run(&["test", "--output=json"]);
    assert_eq!(code, 1, "{out}");
    let events = json_events(&out);
    let outcomes = events_of_kind(&events, "test_outcome");
    assert_eq!(outcomes.len(), 1, "{out}");
    assert_eq!(outcomes[0]["outcome"], serde_json::json!("not_reported"));
    assert_eq!(outcomes[0]["shard"], serde_json::json!(2));
    assert_eq!(outcomes[0]["evidence_complete"], serde_json::json!(false));
    assert!(outcomes[0].get("bazel_status").is_none(), "{out}");
    assert!(outcomes[0].get("cases_total").is_none(), "{out}");
    let error = events_of_kind(&events, "error")
        .into_iter()
        .find(|error| error["code"] == "incomplete_results")
        .expect("incomplete");
    assert_eq!(
        error["message"],
        serde_json::json!(
            "testResult for //a:t (run 1, shard 2, attempt 1) was announced but never reported"
        )
    );
    assert_eq!(
        event(&events, "command_finished")["results_complete"],
        serde_json::json!(false)
    );
}

#[test]
fn a_cancelled_result_keeps_the_bazel_code_and_its_cause() {
    let harness = Harness {
        bazel_code: 8,
        raw_bep: Some(vec![
            serde_json::json!({
                "id": {"testResult": {"label": "//a:t", "run": 1, "shard": 1, "attempt": 1}},
                "aborted": {"reason": "USER_INTERRUPTED"},
            })
            .to_string(),
        ]),
        ..Harness::new("test-cancelled")
    };
    let (code, out, _) = harness.run(&["test", "--output=json"]);
    assert_eq!(code, 8, "{out}");
    let events = json_events(&out);
    let outcome = event(&events, "test_outcome");
    assert_eq!(outcome["outcome"], serde_json::json!("cancelled"));
    assert_eq!(outcome["evidence_complete"], serde_json::json!(false));
    assert_eq!(
        event(&events, "command_finished")["exit_code"],
        serde_json::json!(8)
    );
}

#[test]
fn a_stream_without_bazels_last_event_is_incomplete() {
    let harness = Harness::new("test-unterminated");
    let uri = write_bep_artifact(&harness, "ok.xml", MINIMAL_TEST_XML.as_bytes());
    let harness = Harness {
        unterminated_bep: true,
        raw_bep: Some(vec![test_result_line(
            "//a:t",
            &[(String::from("test.xml"), uri)],
        )]),
        ..harness
    };
    let (code, _, err) = harness.run(&["test", "--output=text"]);
    assert_eq!(code, 1, "{err}");
    assert!(
        err.contains("dx: incomplete_results: build events ended before Bazel's last event"),
        "{err}"
    );
    let (code, out, _) = harness.run(&["test", "--output=json"]);
    assert_eq!(code, 1, "{out}");
    let events = json_events(&out);
    assert!(
        events_of_kind(&events, "test_outcome").is_empty(),
        "the collected result itself stays complete and is counted: {out}"
    );
    assert_eq!(
        event(&events, "command_finished")["tests"],
        serde_json::json!({"passed": 1, "failed": 0, "cached": 0, "no_cases": 0, "skipped": 0})
    );
    assert_eq!(
        event(&events, "command_finished")["results_complete"],
        serde_json::json!(false)
    );
}

#[test]
fn a_summary_without_results_is_its_own_outcome_without_cases() {
    let harness = Harness {
        bazel_code: 1,
        raw_bep: Some(vec![summary_line("//a:broken", "FAILED_TO_BUILD")]),
        ..Harness::new("test-summary-only")
    };
    let (code, out, _) = harness.run(&["test", "--output=json"]);
    assert_eq!(code, 1, "{out}");
    let events = json_events(&out);
    let outcome = event(&events, "test_outcome");
    assert_eq!(outcome["target"], serde_json::json!("//a:broken"));
    assert_eq!(outcome["outcome"], serde_json::json!("failed_to_build"));
    assert_eq!(
        outcome["bazel_status"],
        serde_json::json!("FAILED_TO_BUILD")
    );
    assert_eq!(outcome["evidence_complete"], serde_json::json!(false));
    for field in ["cases_total", "run", "shard", "attempt", "cached"] {
        assert!(outcome.get(field).is_none(), "{field}: {out}");
    }

    let harness = Harness {
        raw_bep: Some(vec![summary_line("//a:t", "PASSED")]),
        ..Harness::new("test-summary-only-pass")
    };
    let (code, _, err) = harness.run(&["test", "--output=text"]);
    assert_eq!(code, 1, "a summary is not a collected result: {err}");
    assert!(
        err.contains("no test result was reported for //a:t (summary PASSED)"),
        "{err}"
    );
}

#[test]
fn a_test_that_fails_to_build_is_its_own_outcome() {
    let configured = |label: &str| {
        serde_json::json!({
            "id": {"targetConfigured": {"label": label}},
            "configured": {"targetKind": "sh_test rule", "testSize": "MEDIUM"},
        })
        .to_string()
    };
    let completed = |label: &str, success: bool| {
        serde_json::json!({
            "id": {"targetCompleted": {"label": label, "configuration": {"id": "cfg1"}}},
            "completed": {"success": success},
        })
        .to_string()
    };
    let harness = Harness::new("test-unbuilt");
    let uri = write_bep_artifact(&harness, "ok.xml", MINIMAL_TEST_XML.as_bytes());
    let harness = Harness {
        bazel_code: 1,
        raw_bep: Some(vec![
            configured("//a:broken"),
            configured("//a:t"),
            serde_json::json!({
                "id": {"targetCompleted": {"label": "//a:broken", "configuration": {"id": "cfg1"}}},
                "completed": {"outputGroup": [{"name": "default", "fileSets": []}]},
            })
            .to_string(),
            completed("//a:t", true),
            test_result_line("//a:t", &[(String::from("test.xml"), uri)]),
        ]),
        ..harness
    };
    let (code, out, _) = harness.run(&["test", "--output=json"]);
    assert_eq!(code, 1, "{out}");
    let events = json_events(&out);
    let outcomes = events_of_kind(&events, "test_outcome");
    assert_eq!(outcomes.len(), 1, "{out}");
    assert_eq!(outcomes[0]["target"], serde_json::json!("//a:broken"));
    assert_eq!(outcomes[0]["outcome"], serde_json::json!("failed_to_build"));
    assert_eq!(outcomes[0]["configuration"], serde_json::json!("cfg1"));
    assert_eq!(outcomes[0]["evidence_complete"], serde_json::json!(false));
    for field in [
        "bazel_status",
        "cases_total",
        "run",
        "shard",
        "attempt",
        "cached",
    ] {
        assert!(outcomes[0].get(field).is_none(), "{field}: {out}");
    }
    let error = events_of_kind(&events, "error")
        .into_iter()
        .find(|error| error["code"] == "incomplete_results")
        .expect("incomplete");
    assert_eq!(error["code"], serde_json::json!("incomplete_results"));
    assert_eq!(
        error["message"],
        serde_json::json!("//a:broken failed to build")
    );
    let finished = event(&events, "command_finished");
    assert_eq!(finished["results_complete"], serde_json::json!(false));

    let harness = Harness {
        bazel_code: 1,
        raw_bep: Some(vec![
            configured("//a:broken"),
            completed("//a:broken", false),
            summary_line("//a:broken", "FAILED_TO_BUILD"),
        ]),
        ..Harness::new("test-unbuilt-summarized")
    };
    let (code, out, _) = harness.run(&["test", "--output=json"]);
    assert_eq!(code, 1, "{out}");
    let events = json_events(&out);
    let outcomes = events_of_kind(&events, "test_outcome");
    assert_eq!(outcomes.len(), 1, "{out}");
    assert_eq!(
        outcomes[0]["bazel_status"],
        serde_json::json!("FAILED_TO_BUILD")
    );
}

#[test]
fn skipped_targets_are_reported_without_blocking_collected_results() {
    let harness = Harness::new("test-skipped");
    let uri = write_bep_artifact(&harness, "ok.xml", MINIMAL_TEST_XML.as_bytes());
    let configured = serde_json::json!({
        "id": {"targetConfigured": {"label": "//a:windows_only"}},
        "configured": {"targetKind": "sh_test rule", "testSize": "SMALL"},
    })
    .to_string();
    let skipped = serde_json::json!({
        "id": {"targetCompleted": {"label": "//a:windows_only", "configuration": {"id": "cfg1"}}},
        "aborted": {"reason": "SKIPPED"},
    })
    .to_string();
    let harness = Harness {
        raw_bep: Some(vec![
            configured.clone(),
            skipped.clone(),
            test_result_line("//a:t", &[(String::from("test.xml"), uri)]),
        ]),
        ..harness
    };
    let (code, out, _) = harness.run(&["test", "--output=json"]);
    assert_eq!(code, 0, "{out}");
    let events = json_events(&out);
    let outcomes = events_of_kind(&events, "test_outcome");
    assert_eq!(outcomes.len(), 1, "{out}");
    assert_eq!(outcomes[0]["target"], serde_json::json!("//a:windows_only"));
    assert_eq!(outcomes[0]["outcome"], serde_json::json!("skipped"));
    assert_eq!(outcomes[0]["configuration"], serde_json::json!("cfg1"));
    assert!(outcomes[0].get("evidence_complete").is_none(), "{out}");
    assert_eq!(
        event(&events, "command_finished")["results_complete"],
        serde_json::json!(true)
    );

    let only_skipped = Harness {
        raw_bep: Some(vec![configured, skipped]),
        ..Harness::new("test-only-skipped")
    };
    let (code, out, _) = only_skipped.run(&["test", "--output=json"]);
    assert_eq!(code, 0, "nothing was selected to run: {out}");
    let events = json_events(&out);
    assert_eq!(
        event(&events, "test_outcome")["outcome"],
        serde_json::json!("skipped")
    );
}

fn filtered_target_lines() -> Vec<String> {
    vec![
        serde_json::json!({
            "id": {"targetConfigured": {"label": "//a:slow"}},
            "configured": {"targetKind": "sh_test rule", "testSize": "LARGE"},
        })
        .to_string(),
        serde_json::json!({
            "id": {"targetCompleted": {"label": "//a:slow", "configuration": {"id": "cfg1"}}},
            "completed": {"success": true},
        })
        .to_string(),
    ]
}

#[test]
fn filtered_targets_are_reported_and_counted() {
    let harness = Harness::new("test-filtered");
    let uri = write_bep_artifact(&harness, "ok.xml", MINIMAL_TEST_XML.as_bytes());
    let mut lines = filtered_target_lines();
    lines.push(test_result_line(
        "//a:t",
        &[(String::from("test.xml"), uri)],
    ));
    let harness = Harness {
        raw_bep: Some(lines),
        ..harness
    };
    let (code, out, _) = harness.run(&["test", "--output=json"]);
    assert_eq!(code, 0, "{out}");
    let events = json_events(&out);
    let outcomes = events_of_kind(&events, "test_outcome");
    assert_eq!(outcomes.len(), 1, "{out}");
    assert_eq!(outcomes[0]["target"], serde_json::json!("//a:slow"));
    assert_eq!(outcomes[0]["outcome"], serde_json::json!("filtered"));
    assert!(outcomes[0].get("evidence_complete").is_none(), "{out}");
    let notices: Vec<_> = events_of_kind(&events, "notice")
        .into_iter()
        .filter(|notice| notice["code"] == serde_json::json!("tests_filtered"))
        .collect();
    assert_eq!(notices.len(), 1, "{out}");
    let finished = event(&events, "command_finished");
    assert_eq!(finished["tests"]["skipped"], serde_json::json!(1), "{out}");
    assert_eq!(finished["tests"]["failed"], serde_json::json!(0), "{out}");
}

#[test]
fn skipped_targets_print_a_summary_line_and_keep_the_exit_code() {
    let harness = Harness::new("test-skipped-notice");
    let uri = write_bep_artifact(&harness, "ok.xml", MINIMAL_TEST_XML.as_bytes());
    let lines = vec![
        serde_json::json!({
            "id": {"targetConfigured": {"label": "//a:skip"}},
            "configured": {"targetKind": "sh_test rule", "testSize": "SMALL"},
        })
        .to_string(),
        serde_json::json!({
            "id": {"targetCompleted": {"label": "//a:skip", "configuration": {"id": "cfg1"}}},
            "aborted": {"reason": "SKIPPED"},
        })
        .to_string(),
        test_result_line("//a:t", &[(String::from("test.xml"), uri)]),
    ];
    let harness = Harness {
        raw_bep: Some(lines),
        ..harness
    };
    let (code, _, err) = harness.run(&["test", "--output=text"]);
    assert_eq!(code, 0, "{err}");
    assert_eq!(err.matches("dx: skipped 1 target ").count(), 1, "{err}");
}

#[test]
fn filtered_targets_print_a_stderr_line_in_text_mode() {
    let harness = Harness::new("test-filtered-text");
    let uri = write_bep_artifact(&harness, "ok.xml", MINIMAL_TEST_XML.as_bytes());
    let mut lines = filtered_target_lines();
    lines.push(test_result_line(
        "//a:t",
        &[(String::from("test.xml"), uri)],
    ));
    let harness = Harness {
        raw_bep: Some(lines),
        ..harness
    };
    let (code, _, err) = harness.run(&["test", "--output=text"]);
    assert_eq!(code, 0, "{err}");
    assert_eq!(err.matches("dx: tests_filtered: ").count(), 1, "{err}");
    assert!(err.contains("//a:slow"), "{err}");
}

#[test]
fn coverage_reports_filtered_targets() {
    let mut harness = coverage_harness("cov-filtered", MINIMAL_LCOV.as_bytes());
    let mut lines = filtered_target_lines();
    lines.extend(harness.raw_bep.take().expect("bep"));
    harness.raw_bep = Some(lines);
    let (_, out, _) = harness.run(&["coverage", "--output=json"]);
    let events = json_events(&out);
    let outcomes = events_of_kind(&events, "test_outcome");
    assert!(
        outcomes
            .iter()
            .any(|outcome| outcome["outcome"] == serde_json::json!("filtered")),
        "{out}"
    );
}

#[test]
fn coverage_without_bazels_last_event_is_incomplete() {
    let mut harness = coverage_harness("cov-unterminated", MINIMAL_LCOV.as_bytes());
    harness.unterminated_bep = true;
    let (code, _, err) = harness.run(&["coverage", "--output=text"]);
    assert_eq!(code, 1, "{err}");
    assert!(
        err.contains("dx: incomplete_results: build events ended before Bazel's last event"),
        "{err}"
    );
}

fn no_collector_configured(label: &str) -> String {
    serde_json::json!({
        "id": {"targetConfigured": {"label": label}},
        "configured": {
            "targetKind": "_zig_forward_test rule",
            "testSize": "SMALL",
            "tag": ["manual", "dx-no-coverage-collector"],
        },
    })
    .to_string()
}

#[test]
fn coverage_fails_when_only_collectorless_tests_ran() {
    let harness = Harness::new("cov-no-collector-only");
    let empty = write_bep_artifact(&harness, "zig.dat", b"");
    let harness = Harness {
        raw_bep: Some(vec![
            no_collector_configured("//z:t"),
            test_result_line("//z:t", &[(String::from("coverage.dat"), empty)]),
        ]),
        ..harness
    };
    let (code, _, err) = harness.run(&["coverage", "--output=text", "--fail-on=warning"]);
    assert_eq!(code, 1, "{err}");
    assert!(
        err.contains("dx: no_coverage_collector: no coverage collector for //z:t"),
        "{err}"
    );
    assert!(!err.contains("incomplete_results"), "{err}");
}

#[test]
fn coverage_fails_when_collectorless_tests_mix_with_lcov() {
    let harness = Harness::new("cov-no-collector-mixed");
    let empty = write_bep_artifact(&harness, "zig2.dat", b"");
    let ok = write_bep_artifact(&harness, "rust.dat", MINIMAL_LCOV.as_bytes());
    let harness = Harness {
        raw_bep: Some(vec![
            no_collector_configured("//z:t"),
            test_result_line("//z:t", &[(String::from("coverage.dat"), empty.clone())]),
            test_result_line("//z:t", &[(String::from("coverage.dat"), empty)]),
            test_result_line("//r:t", &[(String::from("coverage.dat"), ok)]),
        ]),
        ..harness
    };
    let (code, _, err) = harness.run(&[
        "coverage",
        "--output=text",
        "--fail-on=warning",
        "--report=lcov=out.lcov",
    ]);
    assert_eq!(code, 1, "{err}");
    assert_eq!(err.matches("no_coverage_collector").count(), 1, "{err}");
    let document = std::fs::read(harness.workspace.join("out.lcov")).expect("lcov");
    assert_eq!(document, MINIMAL_LCOV.as_bytes());
}

#[test]
fn coverage_skips_collectorless_tests_with_one_line_per_language() {
    let harness = Harness::new("cov-no-collector-skip");
    let empty = write_bep_artifact(&harness, "zig4.dat", b"");
    let ok = write_bep_artifact(&harness, "rust2.dat", MINIMAL_LCOV.as_bytes());
    let harness = Harness {
        raw_bep: Some(vec![
            no_collector_configured("//z:a"),
            no_collector_configured("//z:b"),
            test_result_line("//z:a", &[(String::from("coverage.dat"), empty.clone())]),
            test_result_line("//z:b", &[(String::from("coverage.dat"), empty)]),
            test_result_line("//r:t", &[(String::from("coverage.dat"), ok)]),
        ]),
        ..harness
    };
    let (code, _, err) = harness.run(&["coverage", "--output=text", "--fail-on=error"]);
    assert_eq!(code, 0, "{err}");
    assert_eq!(
        err.matches("dx: skipped 2 zig tests: no coverage collector (#7298)")
            .count(),
        1,
        "{err}"
    );
    assert!(!err.contains("dx: no_coverage_collector"), "{err}");
}

#[test]
fn coverage_ignores_collectorless_tests_without_results() {
    let mut harness = coverage_harness("cov-no-collector-filtered", MINIMAL_LCOV.as_bytes());
    let mut lines = vec![no_collector_configured("//z:skipped")];
    lines.extend(harness.raw_bep.take().expect("bep"));
    harness.raw_bep = Some(lines);
    let (code, _, err) = harness.run(&["coverage", "--output=text"]);
    assert_eq!(code, 0, "{err}");
    assert!(!err.contains("no_coverage_collector"), "{err}");
}

#[test]
fn coverage_no_collector_is_a_json_error_event() {
    let harness = Harness::new("cov-no-collector-json");
    let empty = write_bep_artifact(&harness, "zig3.dat", b"");
    let harness = Harness {
        raw_bep: Some(vec![
            no_collector_configured("//z:t"),
            test_result_line("//z:t", &[(String::from("coverage.dat"), empty)]),
        ]),
        ..harness
    };
    let (code, out, _) = harness.run(&["coverage", "--output=json", "--fail-on=warning"]);
    assert_eq!(code, 1, "{out}");
    let events = json_events(&out);
    assert!(
        events_of_kind(&events, "error")
            .iter()
            .any(|event| event["code"] == serde_json::json!("no_coverage_collector")),
        "{out}"
    );
    assert_eq!(
        event(&events, "command_finished")["results_complete"],
        serde_json::json!(true),
        "{out}"
    );
}

#[test]
fn coverage_reports_kcov_lcov_from_untagged_zig_tests() {
    let harness = Harness::new("cov-zig-kcov");
    let kcov = "SF:zig/calc.zig\nDA:1,2\nDA:6,0\nLF:2\nLH:1\nend_of_record\n";
    let uri = write_bep_artifact(&harness, "kcov.dat", kcov.as_bytes());
    let configured = serde_json::json!({
        "id": {"targetConfigured": {"label": "//z:t"}},
        "configured": {"targetKind": "_zig_forward_test rule", "testSize": "SMALL", "tag": []},
    })
    .to_string();
    let harness = Harness {
        raw_bep: Some(vec![
            configured,
            test_result_line("//z:t", &[(String::from("coverage.dat"), uri)]),
        ]),
        ..harness
    };
    let (code, _, err) = harness.run(&[
        "coverage",
        "--output=text",
        "--fail-on=warning",
        "--report=lcov=out.lcov",
    ]);
    assert_eq!(code, 0, "{err}");
    assert!(!err.contains("no coverage collector"), "{err}");
    let document = std::fs::read(harness.workspace.join("out.lcov")).expect("lcov");
    assert_eq!(document, kcov.as_bytes());
}

#[test]
fn coverage_reports_kcov_lcov_from_untagged_fortran_tests() {
    let harness = Harness::new("cov-fortran-kcov");
    let kcov = "SF:fortran/calc.f90\nDA:1,2\nDA:6,0\nLF:2\nLH:1\nend_of_record\n";
    let uri = write_bep_artifact(&harness, "kcov.dat", kcov.as_bytes());
    let configured = serde_json::json!({
        "id": {"targetConfigured": {"label": "//f:t"}},
        "configured": {"targetKind": "_fortran_forward_test rule", "testSize": "SMALL", "tag": []},
    })
    .to_string();
    let harness = Harness {
        raw_bep: Some(vec![
            configured,
            test_result_line("//f:t", &[(String::from("coverage.dat"), uri)]),
        ]),
        ..harness
    };
    let (code, _, err) = harness.run(&[
        "coverage",
        "--output=text",
        "--fail-on=warning",
        "--report=lcov=out.lcov",
    ]);
    assert_eq!(code, 0, "{err}");
    assert!(!err.contains("no coverage collector"), "{err}");
    let document = std::fs::read(harness.workspace.join("out.lcov")).expect("lcov");
    assert_eq!(document, kcov.as_bytes());
}

#[test]
fn coverage_reports_bisect_lcov_from_untagged_ocaml_tests() {
    let harness = Harness::new("cov-ocaml-bisect");
    let lcov = "SF:ocaml/calc.ml\nDA:1,2\nDA:6,0\nLF:2\nLH:1\nend_of_record\n";
    let uri = write_bep_artifact(&harness, "bisect.dat", lcov.as_bytes());
    let configured = serde_json::json!({
        "id": {"targetConfigured": {"label": "//o:t"}},
        "configured": {"targetKind": "_ocaml_forward_test rule", "testSize": "SMALL", "tag": []},
    })
    .to_string();
    let harness = Harness {
        raw_bep: Some(vec![
            configured,
            test_result_line("//o:t", &[(String::from("coverage.dat"), uri)]),
        ]),
        ..harness
    };
    let (code, _, err) = harness.run(&[
        "coverage",
        "--output=text",
        "--fail-on=warning",
        "--report=lcov=out.lcov",
    ]);
    assert_eq!(code, 0, "{err}");
    assert!(!err.contains("no coverage collector"), "{err}");
    let document = std::fs::read(harness.workspace.join("out.lcov")).expect("lcov");
    assert_eq!(document, lcov.as_bytes());
}

#[test]
fn test_ignores_the_no_collector_tag() {
    let harness = Harness::new("test-no-collector");
    let uri = write_bep_artifact(&harness, "zig.xml", MINIMAL_TEST_XML.as_bytes());
    let harness = Harness {
        raw_bep: Some(vec![
            no_collector_configured("//z:t"),
            test_result_line("//z:t", &[(String::from("test.xml"), uri)]),
        ]),
        ..harness
    };
    let (code, _, err) = harness.run(&["test", "--output=text"]);
    assert_eq!(code, 0, "{err}");
    assert!(!err.contains("no_coverage_collector"), "{err}");
}

struct BrokenPipe;

impl std::io::Write for BrokenPipe {
    fn write(&mut self, _buf: &[u8]) -> std::io::Result<usize> {
        Err(std::io::Error::new(
            std::io::ErrorKind::BrokenPipe,
            "broken pipe",
        ))
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Err(std::io::Error::new(
            std::io::ErrorKind::BrokenPipe,
            "broken pipe",
        ))
    }
}

#[test]
fn announce_incomplete_returns_the_stdout_code_when_stdout_breaks() {
    let mut err = Vec::new();
    assert_eq!(
        super::announce_incomplete(
            dx_output::OutputMode::Json,
            &mut BrokenPipe,
            &mut err,
            "no test.xml"
        ),
        Err(dx_process::broken_pipe_code())
    );
}

#[test]
fn coverage_threshold_returns_the_stdout_code_when_stdout_breaks() {
    let mut err = Vec::new();
    let account = dx_lcov::CellAccount::default();
    assert_eq!(
        super::coverage_threshold(
            Some(80),
            Some(&account),
            &super::Sources::default(),
            dx_output::OutputMode::Json,
            &mut BrokenPipe,
            &mut err,
        ),
        Err(dx_process::broken_pipe_code())
    );
}

#[test]
fn an_outcome_that_fails_to_build_ends_the_stream_once() {
    let outcome = dx_output::TestOutcome {
        outcome: "failed".to_owned(),
        ..dx_output::TestOutcome::default()
    };
    let mut out = Vec::new();
    let mut err = Vec::new();
    let ended = super::emit_outcomes(
        dx_output::OutputMode::Json,
        &mut out,
        &mut err,
        &[outcome],
        &[],
    );
    assert_eq!(ended.err(), Some(dx_process::operational_code()));
    let events: Vec<serde_json::Value> = String::from_utf8(out)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    let kinds: Vec<&str> = events
        .iter()
        .map(|event| event["event"].as_str().unwrap())
        .collect();
    assert_eq!(kinds, ["error", "command_finished"], "{events:?}");
    assert_eq!(events[0]["code"], "invalid_result");
}
