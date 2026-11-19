use crate::build_failures::to_event;
use dx_bep::{BuildFailure, StderrRef, collect_build_failures};
use dx_exec_core::common::{check_stdout_write, emit_built, report_failed, write_report_document};
use dx_output::{OutputMode, build_failure_event, report_event};
use dx_plan::args::Invocation;
use dx_plan::reports::PlannedReport;
use serde_json::{Map, Value, json};
use std::io::{BufReader, Read, Write};
use std::path::Path;

const FAILURE_LIMIT: usize = 200;
const STDERR_LIMIT: u64 = 64 * 1024;
const TOTAL_LIMIT: u64 = 1024 * 1024;

struct Capture {
    status: &'static str,
    bytes: u64,
    truncated: bool,
    redacted_lines: u64,
    text: Option<String>,
}

fn is_redacted(line: &str) -> bool {
    let line = line.trim_start();
    line.starts_with("(cd ") || line.starts_with("exec env") || line.contains(" exec env ")
}

fn unavailable(status: &'static str) -> Capture {
    Capture {
        status,
        bytes: 0,
        truncated: false,
        redacted_lines: 0,
        text: None,
    }
}

fn capture_stderr(failure: &BuildFailure, structured: bool, budget: &mut u64) -> Capture {
    let path = match &failure.stderr {
        StderrRef::Remote => return unavailable("remote"),
        StderrRef::Absent => return unavailable("absent"),
        StderrRef::Local(path) => path,
    };
    let Ok(file) = std::fs::File::open(path) else {
        return unavailable("unreadable");
    };
    let size = file.metadata().map_or(0, |meta| meta.len());
    if structured {
        return Capture {
            status: "structured",
            bytes: size,
            ..unavailable("structured")
        };
    }
    let allowed = STDERR_LIMIT.min(*budget);
    if allowed == 0 {
        return Capture {
            bytes: size,
            ..unavailable("omitted_limit")
        };
    }
    let mut bytes = Vec::new();
    if file.take(allowed).read_to_end(&mut bytes).is_err() {
        return unavailable("unreadable");
    }
    *budget -= bytes.len() as u64;
    let truncated = size > bytes.len() as u64;
    let lossy = String::from_utf8_lossy(&bytes);
    let mut redacted_lines = 0;
    let mut kept = Vec::new();
    for line in lossy.lines() {
        if is_redacted(line) {
            redacted_lines += 1;
        } else {
            kept.push(line);
        }
    }
    Capture {
        status: if truncated { "truncated" } else { "captured" },
        bytes: size,
        truncated,
        redacted_lines,
        text: Some(kept.join("\n")),
    }
}

fn record(failure: &BuildFailure, budget: &mut u64) -> Value {
    let event = to_event(failure);
    let structured = event.diagnostics_status == "parsed";
    let mut value = build_failure_event(&event).unwrap_or_else(|_| json!({}));
    let capture = capture_stderr(failure, structured, budget);
    if let Some(map) = value.as_object_mut() {
        map.remove("event");
        let mut stderr = Map::new();
        stderr.insert("status".to_owned(), Value::from(capture.status));
        stderr.insert("bytes".to_owned(), Value::from(capture.bytes));
        stderr.insert("truncated".to_owned(), Value::Bool(capture.truncated));
        stderr.insert(
            "redacted_lines".to_owned(),
            Value::from(capture.redacted_lines),
        );
        if let Some(text) = capture.text {
            stderr.insert("text".to_owned(), Value::String(text));
        }
        map.insert("stderr".to_owned(), Value::Object(stderr));
    }
    value
}

fn document(bep: &Path, bazel_code: i32) -> (String, bool) {
    let mut problem = None;
    let mut records: Vec<Value> = Vec::new();
    let mut omitted = 0usize;
    if bazel_code != 0 {
        match std::fs::File::open(bep) {
            Ok(file) => {
                let collected = collect_build_failures(BufReader::new(file));
                problem = collected.problem;
                let mut budget = TOTAL_LIMIT;
                for failure in &collected.failures {
                    if records.len() >= FAILURE_LIMIT {
                        omitted += 1;
                    } else {
                        records.push(record(failure, &mut budget));
                    }
                }
            }
            Err(error) => problem = Some(format!("build events are unreadable: {error}")),
        }
    }
    let complete = problem.is_none() && omitted == 0;
    let truncated = omitted > 0
        || records.iter().any(|record| {
            matches!(
                record.pointer("/stderr/status").and_then(Value::as_str),
                Some("truncated" | "omitted_limit")
            )
        });
    let mut root = Map::new();
    root.insert("record_version".to_owned(), Value::from(1));
    root.insert("exit_code".to_owned(), Value::from(bazel_code));
    root.insert("complete".to_owned(), Value::Bool(complete));
    root.insert("truncated".to_owned(), Value::Bool(truncated));
    root.insert("failures_omitted".to_owned(), Value::from(omitted));
    if let Some(problem) = problem {
        root.insert("problem".to_owned(), Value::String(problem));
    }
    root.insert("failures".to_owned(), Value::Array(records));
    // a serde_json::Value always serializes
    let text = serde_json::to_string_pretty(&Value::Object(root)).unwrap_or_default();
    (text, complete)
}

/// Writes the planned failure reports and returns whether every one landed.
pub fn write_failure_reports(
    invocation: &Invocation,
    workspace: &Path,
    bep: &Path,
    bazel_code: i32,
    reports: &[PlannedReport],
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<bool, i32> {
    if reports.is_empty() {
        return Ok(true);
    }
    let (text, complete) = document(bep, bazel_code);
    let mut all_ok = true;
    for planned in reports {
        if let Err(error) = write_report_document(out, workspace, &planned.destination, &text) {
            all_ok = false;
            report_failed(
                out,
                err,
                invocation.output,
                &format!("failed to write failures report to {error}"),
            )?;
            continue;
        }
        if invocation.output == OutputMode::Json {
            emit_built(
                out,
                report_event("failures", planned.destination.display(), complete),
            )?;
        } else if matches!(invocation.output, OutputMode::Text { .. })
            && planned.destination != dx_plan::reports::Destination::Stdout
        {
            check_stdout_write(writeln!(
                out,
                "Wrote failures report to {}.",
                planned.destination.display()
            ))?;
        }
    }
    Ok(all_ok)
}
