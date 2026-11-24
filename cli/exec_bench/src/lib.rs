#![deny(
    clippy::indexing_slicing,
    clippy::let_underscore_must_use,
    clippy::map_err_ignore,
    clippy::string_slice,
    clippy::unused_result_ok,
    clippy::wildcard_enum_match_arm
)]

use dx_bep::{TestResultRecord, collect_test_events};
use dx_error::DxError;
use dx_exec_core::common::{
    CODE_INVALID_BEP, CODE_REPORT_FAILED, CODE_UNREADABLE_BEP, Env, check_stdout_write, kept_child,
    operational, pre_exec, report_dx_error, resolve_failed, run_bazel, temp_path_not_utf8,
};
use dx_exec_core::failure::CODE_NO_TESTS;
use dx_output::OutputMode;
use dx_plan::args::Invocation;
use dx_plan::plan::{WorkflowVerb, bep_path, plan_workflow};
use dx_plan::resolve::{
    ResolvedScope, SelectionContext, exclude, resolve_for_test_with_selection, run_label_query_file,
};
use dx_process::Scope;
use serde::{Deserialize, Serialize};
use std::io::BufReader;
use std::path::{Path, PathBuf};

const SCHEMA: &str = "dx-bench/v1";
const BENCH_FILE: &str = "dx-bench.json";
const CODE_BENCH_RESULTS: &str = "bench_results";
const CODE_BENCH_COMPARE: &str = "bench_compare";
const CODE_BENCH_REGRESSION: &str = "bench_regression";

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

#[derive(Debug, Default)]
struct Baseline {
    rows: Vec<(Option<String>, Measurement)>,
}

#[derive(Debug, Deserialize)]
struct BaselineFile {
    schema: String,
    #[serde(default)]
    results: Vec<Measurement>,
    #[serde(default)]
    targets: Vec<TargetResultsIn>,
}

#[derive(Debug, Deserialize)]
struct TargetResultsIn {
    label: String,
    results: Vec<Measurement>,
}

impl Baseline {
    fn parse(bytes: &[u8]) -> Result<Baseline, DxError> {
        let file: BaselineFile = serde_json::from_slice(bytes)
            .map_err(|error| DxError::failure(format!("invalid baseline: {error}")))?;
        if file.schema != SCHEMA {
            return Err(DxError::failure(format!(
                "invalid baseline: schema is {:?}, want {SCHEMA:?}",
                file.schema
            )));
        }
        let mut rows: Vec<(Option<String>, Measurement)> =
            file.results.into_iter().map(|row| (None, row)).collect();
        for target in file.targets {
            for row in target.results {
                rows.push((Some(target.label.clone()), row));
            }
        }
        Ok(Baseline { rows })
    }

    fn find(&self, label: &str, name: &str) -> Option<&Measurement> {
        self.rows
            .iter()
            .find(|(seen, row)| row.name == name && seen.as_deref() == Some(label))
            .or_else(|| {
                self.rows
                    .iter()
                    .find(|(seen, row)| row.name == name && seen.is_none())
            })
            .map(|(_, row)| row)
    }
}

struct Delta {
    baseline: f64,
    percent: f64,
    regression_beyond_noise: bool,
}

fn delta(row: &Measurement, base: &Measurement) -> Option<Delta> {
    if base.mean <= 0.0 {
        return None;
    }
    let change = row.mean - base.mean;
    let noise = 2.0 * row.stddev.max(base.stddev);
    Some(Delta {
        baseline: base.mean,
        percent: change / base.mean * 100.0,
        regression_beyond_noise: change > noise,
    })
}

fn load_baseline(
    reference: &str,
    workspace: &Path,
    gh: &Path,
    scratch: &Path,
) -> Result<Baseline, DxError> {
    let path = workspace.join(reference);
    if path.is_file() {
        let bytes = std::fs::read(&path).map_err(|error| {
            DxError::from_io(format!("cannot read {}: {error}", path.display()), &error)
        })?;
        return Baseline::parse(&bytes);
    }
    let bytes = download_baseline(reference, gh, scratch)?;
    Baseline::parse(&bytes)
}

fn gh_output(gh: &Path, args: &[&str]) -> Result<String, DxError> {
    dx_process::capture::run_checked(
        std::process::Command::new(gh).args(args),
        dx_process::capture::DEFAULT_TIMEOUT,
    )
    .map(|output| output.stdout_text())
    .map_err(|error| DxError::infra(error.to_string()))
}

fn download_baseline(reference: &str, gh: &Path, scratch: &Path) -> Result<Vec<u8>, DxError> {
    let platform = dx_plan::platform::platform_key(std::env::consts::OS, std::env::consts::ARCH)
        .ok_or_else(|| DxError::failure("this platform has no dx-bench artifact"))?;
    let listing = gh_output(
        gh,
        &[
            "run",
            "list",
            "--branch",
            reference,
            "--limit",
            "1",
            "--json",
            "databaseId",
            "--jq",
            ".[0].databaseId",
        ],
    )?;
    let run = listing.trim();
    if run.is_empty() || run == "null" {
        return Err(DxError::failure(format!(
            "no workflow run found on {reference:?}"
        )));
    }
    let name = format!("dx-bench-{platform}");
    let dest = scratch.join("baseline");
    let dest_text = dest
        .to_str()
        .ok_or_else(|| DxError::failure("the scratch path is not UTF-8"))?;
    gh_output(
        gh,
        &["run", "download", run, "--name", &name, "--dir", dest_text],
    )?;
    let mut files: Vec<_> = std::fs::read_dir(&dest)
        .map_err(|error| {
            DxError::from_io(format!("cannot read the {name} artifact: {error}"), &error)
        })?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .collect();
    files.sort();
    let file = files
        .iter()
        .find(|path| path.file_name().is_some_and(|name| name == "last.json"))
        .or_else(|| files.first())
        .ok_or_else(|| DxError::failure(format!("the {name} artifact holds no json file")))?;
    std::fs::read(file).map_err(|error| {
        DxError::from_io(format!("cannot read {}: {error}", file.display()), &error)
    })
}

fn bench_expression(resolved: &ResolvedScope) -> String {
    format!(
        "attr(tags, \"[\\[ ]dx-bench[,\\]]\", tests({}))",
        exclude::query_set(resolved)
    )
}

fn parse_bench_file(bytes: &[u8]) -> Result<Vec<Measurement>, DxError> {
    let file: BenchFile = serde_json::from_slice(bytes)
        .map_err(|error| DxError::failure(format!("invalid {BENCH_FILE}: {error}")))?;
    if file.schema != SCHEMA {
        return Err(DxError::failure(format!(
            "invalid {BENCH_FILE}: schema is {:?}, want {SCHEMA:?}",
            file.schema
        )));
    }
    Ok(file.results)
}

const MAX_BENCH_BYTES: u64 = 16 << 20;

fn read_result(result: &TestResultRecord) -> Result<Vec<Measurement>, DxError> {
    let zip = result
        .files
        .iter()
        .find(|file| file.name.ends_with("outputs.zip"))
        .ok_or_else(|| DxError::failure(format!("{BENCH_FILE} was not written")))?;
    let bytes = dx_archive::read_zip_member(&zip.exec_path, BENCH_FILE, MAX_BENCH_BYTES)
        .map_err(|error| DxError::infra(format!("cannot read the test outputs: {error}")))?
        .ok_or_else(|| DxError::failure(format!("{BENCH_FILE} was not written")))?;
    parse_bench_file(&bytes)
}

fn render_table(label: &str, results: &[Measurement], baseline: Option<&Baseline>) -> String {
    let mut head: Vec<String> = ["name", "mean", "stddev", "unit"]
        .map(str::to_owned)
        .to_vec();
    if baseline.is_some() {
        head.push("baseline".to_owned());
        head.push("delta %".to_owned());
    }
    let rows: Vec<Vec<String>> = results
        .iter()
        .map(|row| {
            let mut cells = vec![
                row.name.clone(),
                row.mean.to_string(),
                row.stddev.to_string(),
                row.unit.clone(),
            ];
            if let Some(baseline) = baseline {
                let found = baseline
                    .find(label, &row.name)
                    .and_then(|base| delta(row, base));
                if let Some(found) = found {
                    cells.push(found.baseline.to_string());
                    cells.push(format!("{:+.1}", found.percent));
                } else {
                    cells.push("-".to_owned());
                    cells.push("-".to_owned());
                }
            }
            cells
        })
        .collect();
    let mut widths = vec![0; head.len()];
    for row in std::iter::once(&head).chain(rows.iter()) {
        for (width, cell) in widths.iter_mut().zip(row) {
            *width = (*width).max(cell.chars().count());
        }
    }
    let mut text = format!("{label}\n");
    for row in std::iter::once(&head).chain(rows.iter()) {
        let line: Vec<String> = row
            .iter()
            .zip(&widths)
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

fn add_deltas(document: &mut serde_json::Value, baseline: &Baseline) {
    let Some(targets) = document["targets"].as_array_mut() else {
        return;
    };
    for target in targets {
        let label = target["label"].as_str().unwrap_or_default().to_owned();
        let Some(results) = target["results"].as_array_mut() else {
            continue;
        };
        for result in results {
            let Ok(row) = serde_json::from_value::<Measurement>(result.clone()) else {
                continue;
            };
            let found = baseline
                .find(&label, &row.name)
                .and_then(|base| delta(&row, base));
            if let Some(found) = found {
                result["baseline"] = found.baseline.into();
                result["delta_pct"] = found.percent.into();
            }
        }
    }
}

fn regressions(targets: &[TargetResults], baseline: &Baseline, limit: u32) -> Vec<String> {
    let mut found = Vec::new();
    for target in targets {
        for row in &target.results {
            let Some(base) = baseline.find(&target.label, &row.name) else {
                continue;
            };
            let Some(found_delta) = delta(row, base) else {
                continue;
            };
            if found_delta.percent > f64::from(limit) && found_delta.regression_beyond_noise {
                found.push(format!(
                    "{}: {} is {:.1}% slower than the baseline",
                    target.label, row.name, found_delta.percent
                ));
            }
        }
    }
    found
}

fn write_last(workspace: &Path, document: &serde_json::Value) -> std::io::Result<()> {
    let dir = dx_path::state::local_dir(workspace).join(dx_path::state::BENCH);
    std::fs::create_dir_all(&dir)?;
    let mut text = serde_json::to_string_pretty(document).map_err(std::io::Error::other)?;
    text.push('\n');
    dx_atomic_fs::write_atomic(&dir.join("last.json"), text.as_bytes())
}

pub fn execute_bench(invocation: &Invocation, mut env: Env<'_>) -> i32 {
    if invocation.bench_regression.is_some() && invocation.bench_compare.is_none() {
        return operational(
            invocation,
            env.out,
            env.err,
            CODE_BENCH_COMPARE,
            "--max-regression needs --compare",
        );
    }
    let baseline = match load_compare_baseline(invocation, &mut env) {
        Ok(baseline) => baseline,
        Err(exit) => return exit,
    };
    let (plan, bep) = match plan_bench(invocation, &mut env) {
        Ok(planned) => planned,
        Err(exit) => return exit,
    };
    let Env {
        workspace,
        runner,
        query_runner,
        out,
        err,
        ..
    } = env;
    if invocation.chatty()
        && let Err(exit) = check_stdout_write(writeln!(out, "{}", plan.summary))
    {
        return exit;
    }
    let bazel_code = match run_bazel(invocation, out, err, workspace, runner, &plan.argv, &[]) {
        Ok(code) => code,
        Err(exit) => {
            dx_atomic_fs::discard_file(&bep);
            return exit;
        }
    };
    let events = std::fs::File::open(&bep)
        .map_err(|error| format!("failed to read build events: {error}"))
        .and_then(|file| {
            collect_test_events(BufReader::new(file), || {
                Some(dx_exec_test_reports::output_locations(
                    invocation,
                    workspace,
                    WorkflowVerb::Bench,
                    query_runner,
                ))
            })
            .map_err(|error| format!("invalid build events: {error}"))
        });
    dx_atomic_fs::discard_file(&bep);
    let events = match events {
        Ok(events) => events,
        Err(message) if bazel_code == 0 => {
            return operational(invocation, out, err, CODE_INVALID_BEP, &message);
        }
        Err(_) => return bazel_code,
    };
    let (targets, problems) = passed_targets(&events.results);
    let mut document = merged_document(&targets);
    if let Err(error) = write_last(workspace, &document) {
        return operational(
            invocation,
            out,
            err,
            CODE_REPORT_FAILED,
            &format!("could not write .dx/local/bench/last.json: {error}"),
        );
    }
    if let Some(baseline) = &baseline {
        add_deltas(&mut document, baseline);
    }
    let shown = if invocation.output == OutputMode::Json {
        format!("{document}\n")
    } else {
        targets
            .iter()
            .map(|target| render_table(&target.label, &target.results, baseline.as_ref()))
            .collect::<Vec<_>>()
            .join("\n")
    };
    if let Err(exit) = check_stdout_write(out.write_all(shown.as_bytes())) {
        return kept_child(err, bazel_code, exit);
    }
    for problem in &problems {
        dx_output::write_stderr(err, format_args!("dx: {CODE_BENCH_RESULTS}: {problem}\n"));
    }
    let slower = match (&baseline, invocation.bench_regression) {
        (Some(baseline), Some(limit)) => regressions(&targets, baseline, limit),
        _ => Vec::new(),
    };
    for line in &slower {
        dx_output::write_stderr(err, format_args!("dx: {CODE_BENCH_REGRESSION}: {line}\n"));
    }
    if bazel_code == 0 && (!problems.is_empty() || !slower.is_empty()) {
        return 1;
    }
    bazel_code
}

fn load_compare_baseline(
    invocation: &Invocation,
    env: &mut Env<'_>,
) -> Result<Option<Baseline>, i32> {
    let Some(reference) = &invocation.bench_compare else {
        return Ok(None);
    };
    let workspace = env.workspace;
    let loaded = tempfile::tempdir()
        .map_err(|error| DxError::from_io(error.to_string(), &error))
        .and_then(|scratch| load_baseline(reference, workspace, Path::new("gh"), scratch.path()));
    match loaded {
        Ok(baseline) => Ok(Some(baseline)),
        Err(error) => {
            let error = DxError::new(
                error.kind(),
                format!("cannot compare with {reference:?}: {error}"),
            );
            Err(report_dx_error(
                invocation.output,
                env.out,
                env.err,
                CODE_BENCH_COMPARE,
                &error,
            ))
        }
    }
}

fn plan_bench(
    invocation: &Invocation,
    env: &mut Env<'_>,
) -> Result<(dx_plan::plan::BuildPlan, PathBuf), i32> {
    let workspace = env.workspace;
    let selection = SelectionContext::unconfigured_with_options(
        invocation.raw_bazel_options(),
        &invocation.bazel_startup_options,
    );
    let resolved = resolve_for_test_with_selection(
        &invocation.targets,
        workspace,
        env.query_runner,
        &invocation.bazel_startup_options,
        &selection,
    )
    .map_err(|error| resolve_failed(invocation.output, env.out, env.err, &error))?;
    let tests = run_label_query_file(
        &bench_expression(&resolved),
        "dx-bench tests",
        workspace,
        env.query_runner,
        &invocation.bazel_startup_options,
    )
    .map_err(|error| resolve_failed(invocation.output, env.out, env.err, &error))?;
    if tests.is_empty() {
        return Err(operational(
            invocation,
            env.out,
            env.err,
            CODE_NO_TESTS,
            "no test tagged dx-bench matches the scope",
        ));
    }
    let bep = bep_path(env.temp_dir, env.pid, env.nonce);
    let Some(bep_text) = bep.to_str().map(ToOwned::to_owned) else {
        return Err(operational(
            invocation,
            env.out,
            env.err,
            CODE_UNREADABLE_BEP,
            &temp_path_not_utf8(),
        ));
    };
    let scope = ResolvedScope {
        scope: Scope::Labels(tests.clone()),
        targets: tests,
        excluded: Vec::new(),
        kept: Vec::new(),
    };
    let plan = plan_workflow(
        WorkflowVerb::Bench,
        &scope,
        &invocation.quiet_bazel_options(),
        Some(&bep_text),
        Some(invocation.build_mode()),
        &invocation.bazel_startup_options,
    )
    .map_err(|error| pre_exec(invocation.output, env.out, env.err, &error.to_string()))?;
    Ok((plan, bep))
}

fn passed_targets(results: &[TestResultRecord]) -> (Vec<TargetResults>, Vec<String>) {
    let mut targets: Vec<TargetResults> = Vec::new();
    let mut problems: Vec<String> = Vec::new();
    for result in results
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
    (targets, problems)
}

#[cfg(test)]
mod tests {
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
        let mut resolved = ResolvedScope {
            scope: Scope::Labels(vec!["//app:b".to_owned()]),
            targets: vec!["//app:b".to_owned()],
            excluded: Vec::new(),
            kept: Vec::new(),
        };
        assert_eq!(
            bench_expression(&resolved),
            "attr(tags, \"[\\[ ]dx-bench[,\\]]\", tests(set(\"//app:b\")))"
        );
        resolved.targets = vec!["//...".to_owned()];
        resolved.excluded = vec!["//third_party/...".to_owned()];
        assert_eq!(
            bench_expression(&resolved),
            "attr(tags, \"[\\[ ]dx-bench[,\\]]\", tests((set(\"//...\") except set(\"//third_party/...\"))))"
        );
    }

    #[test]
    fn parse_rejects_other_schemas_and_bad_rows() {
        let good = br#"{"schema":"dx-bench/v1","results":[{"name":"a","unit":"ms","mean":1.5,"stddev":0.1,"iterations":3}]}"#;
        assert_eq!(parse_bench_file(good).expect("parse").len(), 1);
        let other = br#"{"schema":"dx-bench/v2","results":[]}"#;
        assert!(
            parse_bench_file(other)
                .unwrap_err()
                .message()
                .contains("schema")
        );
        let bad = br#"{"schema":"dx-bench/v1","results":[{"name":"a"}]}"#;
        assert!(
            parse_bench_file(bad)
                .unwrap_err()
                .message()
                .starts_with("invalid dx-bench.json")
        );
    }

    #[test]
    fn table_aligns_columns_under_the_label() {
        let text = render_table(
            "//a:b",
            &[row("parse", 1.5), row("render_long", 20.0)],
            None,
        );
        assert_eq!(
            text,
            "//a:b\nname         mean  stddev  unit\nparse        1.5   0.5     ms\nrender_long  20    0.5     ms\n"
        );
    }

    #[test]
    fn baseline_reads_merged_documents_by_label() {
        let merged = br#"{"schema":"dx-bench/v1","targets":[{"label":"//a:b","results":[{"name":"x","unit":"ms","mean":2.0,"stddev":0.1,"iterations":1}]}]}"#;
        let baseline = Baseline::parse(merged).expect("parse");
        assert!(baseline.find("//a:b", "x").is_some());
        assert!(baseline.find("//a:c", "x").is_none());
        assert!(Baseline::parse(br#"{"schema":"other"}"#).is_err());
    }
}
