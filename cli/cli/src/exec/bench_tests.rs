use super::test_support::{Harness, write_bep_artifact};

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = !0u32;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xEDB8_8320 & (!(crc & 1)).wrapping_add(1));
        }
    }
    !crc
}

fn zip_of(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut zip = Vec::new();
    let mut central = Vec::new();
    for (name, content) in entries {
        let crc = crc32(content);
        let size = u32::try_from(content.len()).unwrap_or(u32::MAX);
        let offset = u32::try_from(zip.len()).unwrap_or(u32::MAX);
        zip.extend(0x0403_4b50u32.to_le_bytes());
        zip.extend([20, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        zip.extend(crc.to_le_bytes());
        zip.extend(size.to_le_bytes());
        zip.extend(size.to_le_bytes());
        zip.extend(u16::try_from(name.len()).unwrap_or(u16::MAX).to_le_bytes());
        zip.extend([0, 0]);
        zip.extend(name.as_bytes());
        zip.extend(*content);
        central.extend(0x0201_4b50u32.to_le_bytes());
        central.extend([20, 0, 20, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        central.extend(crc.to_le_bytes());
        central.extend(size.to_le_bytes());
        central.extend(size.to_le_bytes());
        central.extend(u16::try_from(name.len()).unwrap_or(u16::MAX).to_le_bytes());
        central.extend([0; 12]);
        central.extend(offset.to_le_bytes());
        central.extend(name.as_bytes());
    }
    let offset = u32::try_from(zip.len()).unwrap_or(u32::MAX);
    zip.extend(&central);
    zip.extend(0x0605_4b50u32.to_le_bytes());
    zip.extend([0, 0, 0, 0]);
    zip.extend(
        u16::try_from(entries.len())
            .unwrap_or(u16::MAX)
            .to_le_bytes(),
    );
    zip.extend(
        u16::try_from(entries.len())
            .unwrap_or(u16::MAX)
            .to_le_bytes(),
    );
    zip.extend((u32::try_from(central.len()).unwrap_or(u32::MAX)).to_le_bytes());
    zip.extend(offset.to_le_bytes());
    zip.extend([0, 0]);
    zip
}

fn zip_with(name: &str, content: &[u8]) -> Vec<u8> {
    zip_of(&[(name, content)])
}

const HAND_MADE: &str = r#"{"schema":"dx-bench/v1","results":[{"name":"parse","unit":"ms","mean":1.5,"stddev":0.25,"iterations":50}]}"#;

fn bench_harness(name: &str, outputs: Option<Vec<u8>>) -> Harness {
    let harness = Harness::new(name);
    harness.query.script_stdout("//app:parse_bench\n");
    let files: Vec<(String, String)> = outputs
        .map(|bytes| {
            (
                "test.outputs__outputs.zip".to_owned(),
                write_bep_artifact(&harness, "outputs.zip", &bytes),
            )
        })
        .into_iter()
        .collect();
    let line = serde_json::json!({
        "id": {"testResult": {"label": "//app:parse_bench", "run": 1, "shard": 1, "attempt": 1}},
        "testResult": {
            "status": "PASSED",
            "testActionOutput": files
                .iter()
                .map(|(name, uri)| serde_json::json!({"name": name, "uri": uri}))
                .collect::<Vec<_>>(),
        },
    })
    .to_string();
    Harness {
        raw_bep: Some(vec![line]),
        ..harness
    }
}

#[test]
fn bench_prints_the_table_and_writes_last_json() {
    let harness = bench_harness(
        "bench-table",
        Some(zip_with("dx-bench.json", HAND_MADE.as_bytes())),
    );
    let (code, out, err) = harness.run(&["bench", "//..."]);
    assert_eq!(code, 0, "{err}");
    assert!(
        out.contains("//app:parse_bench\nname   mean  stddev  unit\nparse  1.5   0.25    ms\n"),
        "{out}"
    );
    let expressions = harness.query.expressions.borrow();
    assert!(expressions[0].starts_with("attr(tags, "), "{expressions:?}");
    let argv = harness.build_argv.borrow();
    let bazel = argv
        .iter()
        .find(|argv| argv.contains(&"test".to_owned()))
        .expect("bazel test");
    for flag in [
        "--runs_per_test=1",
        "--nocache_test_results",
        "--local_test_jobs=1",
        "--test_env=DX_BENCH=1",
    ] {
        assert!(bazel.contains(&flag.to_owned()), "{flag} in {bazel:?}");
    }
    assert!(bazel.contains(&"//app:parse_bench".to_owned()), "{bazel:?}");
    let last = std::fs::read_to_string(harness.workspace.join(".dx/local/bench/last.json"))
        .expect("last.json");
    let last: serde_json::Value = serde_json::from_str(&last).expect("json");
    assert_eq!(last["schema"], "dx-bench/v1");
    assert_eq!(last["targets"][0]["label"], "//app:parse_bench");
    assert_eq!(last["targets"][0]["results"][0]["mean"], 1.5);
}

#[test]
fn bench_reads_bytestream_test_outputs_through_info_roots() {
    let harness = Harness::new("bench-bytestream");
    harness.query.script_stdout("//app:parse_bench\n");
    let testlogs = harness.temp.join("testlogs");
    harness
        .query
        .script_info(&format!("bazel-testlogs: {}\n", testlogs.display()));
    let dir = testlogs
        .join("app")
        .join("parse_bench")
        .join("test.outputs");
    std::fs::create_dir_all(&dir).expect("testlogs dir");
    std::fs::write(
        dir.join("outputs.zip"),
        zip_with("dx-bench.json", HAND_MADE.as_bytes()),
    )
    .expect("outputs.zip");
    let line = serde_json::json!({
        "id": {"testResult": {"label": "//app:parse_bench", "run": 1, "shard": 1, "attempt": 1}},
        "testResult": {
            "status": "PASSED",
            "testActionOutput": [{
                "name": "test.outputs__outputs.zip",
                "uri": "bytestream://remote.buildbuddy.io/blobs/abc/10",
            }],
        },
    })
    .to_string();
    let harness = Harness {
        raw_bep: Some(vec![line]),
        ..harness
    };
    let (code, out, err) = harness.run(&["bench"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("parse  1.5"), "{out}");
    assert!(harness.workspace.join(".dx/local/bench/last.json").exists());
}

#[test]
fn bench_json_output_is_the_merged_document() {
    let harness = bench_harness(
        "bench-json",
        Some(zip_with("dx-bench.json", HAND_MADE.as_bytes())),
    );
    let (code, out, err) = harness.run(&["bench", "--output=json"]);
    assert_eq!(code, 0, "{err}");
    let document: serde_json::Value = serde_json::from_str(out.trim()).expect("one json document");
    assert_eq!(document["targets"][0]["results"][0]["name"], "parse");
}

#[test]
fn bench_fails_when_a_passing_test_wrote_no_file() {
    let harness = bench_harness("bench-missing", None);
    let (code, _, err) = harness.run(&["bench"]);
    assert_eq!(code, 1, "{err}");
    assert!(
        err.contains("//app:parse_bench: dx-bench.json was not written"),
        "{err}"
    );
}

fn doc(mean: f64) -> String {
    format!(
        r#"{{"schema":"dx-bench/v1","results":[{{"name":"parse","unit":"ms","mean":{mean},"stddev":0.1,"iterations":50}}]}}"#
    )
}

fn compare_run(name: &str, current: f64, baseline: f64, fail_on: &str) -> (i32, String, String) {
    let harness = bench_harness(
        name,
        Some(zip_with("dx-bench.json", doc(current).as_bytes())),
    );
    std::fs::write(harness.workspace.join("a.json"), doc(baseline)).expect("baseline");
    harness.run(&["bench", "--compare", "a.json", fail_on, "--output=json"])
}

#[test]
fn a_thirty_percent_slowdown_fails_at_twenty_and_passes_at_fifty() {
    let (code, out, err) = compare_run("bench-cmp-20", 13.0, 10.0, "--max-regression=20");
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("parse is 30.0% slower"), "{err}");
    let document: serde_json::Value = serde_json::from_str(out.trim()).expect("json");
    let result = &document["targets"][0]["results"][0];
    assert_eq!(result["baseline"], 10.0);
    assert!((result["delta_pct"].as_f64().expect("delta") - 30.0).abs() < 1e-9);
    let (code, _, err) = compare_run("bench-cmp-50", 13.0, 10.0, "--max-regression=50");
    assert_eq!(code, 0, "{err}");
}

#[test]
fn a_change_inside_two_stddevs_is_not_a_regression() {
    let (code, _, err) = compare_run("bench-cmp-noise", 10.1, 10.0, "--max-regression=0");
    assert_eq!(code, 0, "{err}");
}

#[test]
fn compare_adds_baseline_columns_to_the_table() {
    let harness = bench_harness(
        "bench-cmp-table",
        Some(zip_with("dx-bench.json", doc(13.0).as_bytes())),
    );
    std::fs::write(harness.workspace.join("a.json"), doc(10.0)).expect("baseline");
    let (code, out, err) = harness.run(&["bench", "--compare", "a.json"]);
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("baseline  delta %"), "{out}");
    assert!(out.contains("10        +30.0"), "{out}");
}

#[test]
fn regression_gate_needs_a_baseline() {
    let harness = bench_harness("bench-cmp-none", None);
    let (code, _, err) = harness.run(&["bench", "--max-regression=20"]);
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("needs --compare"), "{err}");
}

#[test]
fn bench_without_tagged_tests_is_no_tests() {
    let harness = Harness::new("bench-none");
    harness.query.script_stdout("");
    let (code, _, err) = harness.run(&["bench"]);
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("no_tests"), "{err}");
    assert!(harness.build_argv.borrow().is_empty());
}
