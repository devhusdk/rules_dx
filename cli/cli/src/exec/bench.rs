use super::common::*;
use super::failure::CODE_NO_TESTS;
use dx_bep::{collect_test_events, TestResultRecord};
use dx_output::OutputMode;
use dx_plan::args::Invocation;
use dx_plan::plan::{bep_path, plan_workflow, WorkflowVerb};
use dx_plan::resolve::{
    resolve_for_test_with_selection, run_label_query_file, set_expression, ResolvedScope,
    SelectionContext,
};
use dx_process::Scope;
use serde::{Deserialize, Serialize};
use std::io::BufReader;
use std::path::Path;

const SCHEMA: &str = "dx-bench/v1";
const BENCH_FILE: &str = "dx-bench.json";
const CODE_BENCH_RESULTS: &str = "bench_results";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Measurement {
    name: String,
    unit: String,
    mean: f64,
    stddev: f64,
    iterations: u64,
}

#[derive(Debug, Deserialize)]
struct BenchFile {
    schema: String,
    results: Vec<Measurement>,
}

#[derive(Debug, Serialize)]
struct TargetResults {
    label: String,
    results: Vec<Measurement>,
}

/// The query that keeps the `dx-bench` tagged tests of a scope.
fn bench_expression(targets: &[String]) -> String {
    format!(
        "attr(tags, \"[\\[ ]dx-bench[,\\]]\", tests({}))",
        set_expression(targets)
    )
}

fn parse_bench_file(bytes: &[u8]) -> Result<Vec<Measurement>, String> {
    let file: BenchFile =
        serde_json::from_slice(bytes).map_err(|error| format!("invalid {BENCH_FILE}: {error}"))?;
    if file.schema != SCHEMA {
        return Err(format!(
            "invalid {BENCH_FILE}: schema is {:?}, want {SCHEMA:?}",
            file.schema
        ));
    }
    Ok(file.results)
}

/// Reads `dx-bench.json` out of the undeclared outputs zip one test result reports.
fn read_result(result: &TestResultRecord) -> Result<Vec<Measurement>, String> {
    let zip = result
        .files
        .iter()
        .find(|file| file.name.ends_with("outputs.zip"))
        .ok_or_else(|| format!("{BENCH_FILE} was not written"))?;
    let scratch = tempfile::tempdir().map_err(|error| error.to_string())?;
    let dest = scratch.path().join("outputs");
    dx_archive::extract(
        &zip.exec_path,
        dx_archive::Format::Zip,
        &dest,
        dx_archive::Limits::default(),
    )
    .map_err(|error| format!("cannot read the test outputs: {error}"))?;
    let bytes = std::fs::read(dest.join(BENCH_FILE))
        .map_err(|_| format!("{BENCH_FILE} was not written"))?;
    parse_bench_file(&bytes)
}

fn render_table(label: &str, results: &[Measurement]) -> String {
    let rows: Vec<[String; 4]> = results
        .iter()
        .map(|row| {
            [
                row.name.clone(),
                row.mean.to_string(),
                row.stddev.to_string(),
                row.unit.clone(),
            ]
        })
        .collect();
    let head = ["name", "mean", "stddev", "unit"].map(str::to_owned);
    let mut widths = [0; 4];
    for row in std::iter::once(&head).chain(rows.iter()) {
        for (width, cell) in widths.iter_mut().zip(row) {
            *width = (*width).max(cell.chars().count());
        }
    }
    let mut text = format!("{label}\n");
    for row in std::iter::once(&head).chain(rows.iter()) {
        let line: Vec<String> = row
            .iter()
            .zip(widths)
            .map(|(cell, width)| format!("{cell:<width$}"))
            .collect();
        text.push_str(line.join("  ").trim_end());
        text.push('\n');
    }
    text
}

fn merged_document(targets: &[TargetResults]) -> serde_json::Value {
    serde_json::json!({"schema": SCHEMA, "targets": targets})
}

fn write_last(workspace: &Path, document: &serde_json::Value) -> std::io::Result<()> {
    let dir = workspace.join(".dx").join("bench");
    std::fs::create_dir_all(&dir)?;
    let mut text = serde_json::to_string_pretty(document).map_err(std::io::Error::other)?;
    text.push('\n');
    std::fs::write(dir.join("last.json"), text)
}

pub(crate) fn execute_bench(invocation: &Invocation, env: Env<'_>) -> i32 {
    let Env {
        workspace,
        runner,
        query_runner,
        temp_dir,
        pid,
        nonce,
        out,
        err,
        ..
    } = env;
    let selection = SelectionContext::unconfigured_with_options(
        invocation.raw_bazel_options(),
        &invocation.bazel_startup_options,
    );
    let resolved = match resolve_for_test_with_selection(
        &invocation.targets,
        workspace,
        query_runner,
        &invocation.bazel_startup_options,
        &selection,
    ) {
        Ok(resolved) => resolved,
        Err(error) => return resolve_failed(invocation.output, out, err, &error),
    };
    let tests = match run_label_query_file(
        &bench_expression(&resolved.targets),
        "dx-bench tests",
        workspace,
        query_runner,
        &invocation.bazel_startup_options,
    ) {
        Ok(tests) => tests,
        Err(error) => return resolve_failed(invocation.output, out, &mut *err, &error),
    };
    if tests.is_empty() {
        return operational(
            invocation,
            out,
            err,
            CODE_NO_TESTS,
            "no test tagged dx-bench matches the scope",
        );
    }
    let bep = bep_path(temp_dir, pid, nonce);
    let Some(bep_text) = bep.to_str().map(ToOwned::to_owned) else {
        return operational(
            invocation,
            out,
            err,
            CODE_UNREADABLE_BEP,
            &crate::startup::temp_path_not_utf8(),
        );
    };
    let scope = ResolvedScope {
        scope: Scope::Labels(tests.clone()),
        targets: tests,
    };
    let plan = match plan_workflow(
        WorkflowVerb::Bench,
        &scope,
        &invocation.quiet_bazel_options(),
        Some(&bep_text),
        None,
        Some(invocation.profile()),
        &invocation.bazel_startup_options,
    ) {
        Ok(plan) => plan,
        Err(error) => return pre_exec(invocation.output, out, err, &error.to_string()),
    };
    if invocation.chatty() {
        if let Err(exit) = check_stdout_write(writeln!(out, "{}", plan.summary)) {
            return exit;
        }
    }
    let bazel_code = match run_bazel(invocation, out, err, workspace, runner, &plan.argv, &[]) {
        Ok(code) => code,
        Err(exit) => {
            let _ = std::fs::remove_file(&bep);
            return exit;
        }
    };
    let events = std::fs::File::open(&bep)
        .map_err(|error| format!("failed to read build events: {error}"))
        .and_then(|file| {
            collect_test_events(BufReader::new(file), || None)
                .map_err(|error| format!("invalid build events: {error}"))
        });
    let _ = std::fs::remove_file(&bep);
    let events = match events {
        Ok(events) => events,
        Err(message) if bazel_code == 0 => {
            return operational(invocation, out, err, CODE_INVALID_BEP, &message);
        }
        Err(_) => return bazel_code,
    };
    let mut targets: Vec<TargetResults> = Vec::new();
    let mut problems: Vec<String> = Vec::new();
    for result in events
        .results
        .iter()
        .filter(|result| result.status.as_deref() == Some("PASSED"))
    {
        if targets.iter().any(|seen| seen.label == result.label) {
            continue;
        }
        match read_result(result) {
            Ok(results) => targets.push(TargetResults {
                label: result.label.clone(),
                results,
            }),
            Err(problem) => problems.push(format!("{}: {problem}", result.label)),
        }
    }
    targets.sort_by(|a, b| a.label.cmp(&b.label));
    let document = merged_document(&targets);
    if let Err(error) = write_last(workspace, &document) {
        return operational(
            invocation,
            out,
            err,
            CODE_REPORT_FAILED,
            &format!("could not write .dx/bench/last.json: {error}"),
        );
    }
    let shown = if invocation.output == OutputMode::Json {
        format!("{document}\n")
    } else {
        targets
            .iter()
            .map(|target| render_table(&target.label, &target.results))
            .collect::<Vec<_>>()
            .join("\n")
    };
    if let Err(exit) = check_stdout_write(out.write_all(shown.as_bytes())) {
        return kept_child(err, bazel_code, exit);
    }
    for problem in &problems {
        let _ = writeln!(err, "dx: {CODE_BENCH_RESULTS}: {problem}");
    }
    if bazel_code == 0 && !problems.is_empty() {
        return 1;
    }
    bazel_code
}

#[cfg(test)]
mod tests {
    use super::super::test_support::{write_bep_artifact, Harness};
    use super::*;

    fn row(name: &str, mean: f64) -> Measurement {
        Measurement {
            name: name.to_owned(),
            unit: "ms".to_owned(),
            mean,
            stddev: 0.5,
            iterations: 10,
        }
    }

    #[test]
    fn expression_matches_the_exact_tag_inside_tests() {
        let got = bench_expression(&["//app:b".to_owned()]);
        assert_eq!(
            got,
            "attr(tags, \"[\\[ ]dx-bench[,\\]]\", tests(set(\"//app:b\")))"
        );
    }

    #[test]
    fn parse_rejects_other_schemas_and_bad_rows() {
        let good = br#"{"schema":"dx-bench/v1","results":[{"name":"a","unit":"ms","mean":1.5,"stddev":0.1,"iterations":3}]}"#;
        assert_eq!(parse_bench_file(good).expect("parse").len(), 1);
        let other = br#"{"schema":"dx-bench/v2","results":[]}"#;
        assert!(parse_bench_file(other).unwrap_err().contains("schema"));
        let bad = br#"{"schema":"dx-bench/v1","results":[{"name":"a"}]}"#;
        assert!(parse_bench_file(bad)
            .unwrap_err()
            .starts_with("invalid dx-bench.json"));
    }

    #[test]
    fn table_aligns_columns_under_the_label() {
        let text = render_table("//a:b", &[row("parse", 1.5), row("render_long", 20.0)]);
        assert_eq!(
            text,
            "//a:b\nname         mean  stddev  unit\nparse        1.5   0.5     ms\nrender_long  20    0.5     ms\n"
        );
    }

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

    /// A zip holding one stored file, like the outputs.zip Bazel writes.
    fn zip_with(name: &str, content: &[u8]) -> Vec<u8> {
        let crc = crc32(content);
        let size = content.len() as u32;
        let mut zip = Vec::new();
        let mut local = Vec::new();
        local.extend(0x0403_4b50u32.to_le_bytes());
        local.extend([20, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        local.extend(crc.to_le_bytes());
        local.extend(size.to_le_bytes());
        local.extend(size.to_le_bytes());
        local.extend((name.len() as u16).to_le_bytes());
        local.extend([0, 0]);
        local.extend(name.as_bytes());
        local.extend(content);
        zip.extend(&local);
        let mut central = Vec::new();
        central.extend(0x0201_4b50u32.to_le_bytes());
        central.extend([20, 0, 20, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        central.extend(crc.to_le_bytes());
        central.extend(size.to_le_bytes());
        central.extend(size.to_le_bytes());
        central.extend((name.len() as u16).to_le_bytes());
        central.extend([0; 12]);
        central.extend(0u32.to_le_bytes());
        central.extend(name.as_bytes());
        let offset = zip.len() as u32;
        zip.extend(&central);
        zip.extend(0x0605_4b50u32.to_le_bytes());
        zip.extend([0, 0, 0, 0, 1, 0, 1, 0]);
        zip.extend((central.len() as u32).to_le_bytes());
        zip.extend(offset.to_le_bytes());
        zip.extend([0, 0]);
        zip
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
            Some(zip_with(BENCH_FILE, HAND_MADE.as_bytes())),
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
        let last = std::fs::read_to_string(harness.workspace.join(".dx/bench/last.json"))
            .expect("last.json");
        let last: serde_json::Value = serde_json::from_str(&last).expect("json");
        assert_eq!(last["schema"], SCHEMA);
        assert_eq!(last["targets"][0]["label"], "//app:parse_bench");
        assert_eq!(last["targets"][0]["results"][0]["mean"], 1.5);
    }

    #[test]
    fn bench_json_output_is_the_merged_document() {
        let harness = bench_harness(
            "bench-json",
            Some(zip_with(BENCH_FILE, HAND_MADE.as_bytes())),
        );
        let (code, out, err) = harness.run(&["bench", "--output=json"]);
        assert_eq!(code, 0, "{err}");
        let document: serde_json::Value =
            serde_json::from_str(out.trim()).expect("one json document");
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

    #[test]
    fn bench_without_tagged_tests_is_no_tests() {
        let harness = Harness::new("bench-none");
        harness.query.script_stdout("");
        let (code, _, err) = harness.run(&["bench"]);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("no_tests"), "{err}");
        assert!(harness.build_argv.borrow().is_empty());
    }
}
