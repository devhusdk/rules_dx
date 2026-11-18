//! Prints the tail of each failing test's `test.log` into a GitHub Actions job log, one
//! collapsed group per test, so a reader without artifact access sees why a test failed.

use std::collections::BTreeMap;
use std::io::{BufReader, Write};
use std::path::{Path, PathBuf};

use dx_bep::{OutputLocations, TestResultRecord};
use serde_json::Value;

/// Lines kept from the end of each test.log.
const TAIL_LINES: usize = 80;

/// Bytes kept from those lines, so one runaway line cannot flood the job log.
const TAIL_BYTES: usize = 16 * 1024;

/// Tests printed in full; the rest are only named.
const GROUP_LIMIT: usize = 20;

/// dx `test_outcome` outcomes that count as a pass.
const PASSING_OUTCOMES: [&str; 3] = ["passed", "passed_after_retry", "no_cases"];

/// One failing test result and where its log is.
#[derive(Debug, PartialEq, Eq)]
struct Failure {
    title: String,
    log: Option<PathBuf>,
    /// The tail dx already kept, for when the log is no longer on disk.
    tail: Option<String>,
}

/// Runs `--test-log-tails (--bep FILE | --dx FILE)...`.
///
/// `--bep` reads a Bazel `--build_event_json_file` and `--dx` reads `dx --output=json` events.
/// A missing input is skipped and an unreadable one is reported and skipped: this runs after a
/// step already failed and must not hide that failure behind its own.
///
/// # Errors
///
/// Returns `Err` on a usage error or when stdout cannot be written.
pub fn print(args: &[String]) -> Result<(), String> {
    const USAGE: &str = "usage: producers_bin --test-log-tails (--bep FILE | --dx FILE)...";
    let (pairs, []) = args.as_chunks::<2>() else {
        return Err(USAGE.to_owned());
    };
    if pairs.is_empty() {
        return Err(USAGE.to_owned());
    }
    let workspace = std::env::var_os("BUILD_WORKSPACE_DIRECTORY")
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_default();
    let mut failures = Vec::new();
    let mut without_log = 0;
    for [kind, path] in pairs {
        let path = Path::new(path);
        // A step can fail before Bazel or dx writes its events; that file has no tests to show.
        if !path.exists() {
            continue;
        }
        let read = match kind.as_str() {
            "--bep" => bep_failures(path, &workspace),
            "--dx" => dx_failures(path),
            _ => return Err(USAGE.to_owned()),
        };
        match read {
            Ok((found, missing)) => {
                failures.extend(found);
                without_log += missing;
            }
            Err(problem) => println!("test-log-tails: skipped {}: {problem}", path.display()),
        }
    }
    let mut out = std::io::stdout().lock();
    render(&mut out, &failures, without_log, &stop_token())
        .map_err(|err| format!("test-log-tails: writing stdout: {err}"))
}

/// The failing results of one build event file and the count of those without a test.log.
fn bep_failures(path: &Path, workspace: &Path) -> Result<(Vec<Failure>, usize), String> {
    let file = std::fs::File::open(path).map_err(|err| err.to_string())?;
    let events = dx_bep::collect_test_events(BufReader::new(file), || {
        Some(OutputLocations::new(workspace))
    })
    .map_err(|err| err.to_string())?;
    // Only the last attempt of each run and shard decides it: earlier failed attempts of a
    // flaky test that later passed are not failures.
    let mut last: BTreeMap<(&str, Option<&str>, u32, u32), &TestResultRecord> = BTreeMap::new();
    for result in &events.results {
        let key = (
            result.label.as_str(),
            result.configuration.as_deref(),
            result.run,
            result.shard,
        );
        let entry = last.entry(key).or_insert(result);
        if result.attempt > entry.attempt {
            *entry = result;
        }
    }
    let failing: Vec<&TestResultRecord> = last
        .into_values()
        .filter(|result| result.status.as_deref() != Some("PASSED"))
        .collect();
    let mut found = Vec::new();
    let mut without_log = 0;
    for result in &failing {
        let Some(log) = result.files.iter().find(|file| file.name == "test.log") else {
            without_log += 1;
            continue;
        };
        let repeated = failing
            .iter()
            .filter(|other| other.label == result.label)
            .count()
            > 1;
        let status = result.status.as_deref().unwrap_or("NO_STATUS");
        let title = if repeated {
            format!(
                "{} {status} (run {}, shard {})",
                result.label, result.run, result.shard
            )
        } else {
            format!("{} {status}", result.label)
        };
        found.push(Failure {
            title,
            log: Some(log.exec_path.clone()),
            tail: None,
        });
    }
    Ok((found, without_log))
}

/// The failing final `test_outcome` events of one dx event file and the count without a log.
fn dx_failures(path: &Path) -> Result<(Vec<Failure>, usize), String> {
    let text = std::fs::read_to_string(path).map_err(|err| err.to_string())?;
    let mut found = Vec::new();
    let mut without_log = 0;
    for line in text.lines() {
        // The file is dx's stdout, so anything that is not an event is skipped.
        let Ok(Value::Object(event)) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if event.get("event").and_then(Value::as_str) != Some("test_outcome")
            || event.get("final").and_then(Value::as_bool) == Some(false)
        {
            continue;
        }
        let outcome = event
            .get("outcome")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        if PASSING_OUTCOMES.contains(&outcome) {
            continue;
        }
        let log = event
            .get("log_path")
            .and_then(Value::as_str)
            .map(PathBuf::from);
        let tail = event
            .get("log_tail")
            .and_then(Value::as_str)
            .map(str::to_owned);
        if log.is_none() && tail.is_none() {
            without_log += 1;
            continue;
        }
        let target = event
            .get("target")
            .and_then(Value::as_str)
            .unwrap_or("(unknown target)");
        found.push(Failure {
            title: format!("{target} {}", outcome.to_uppercase()),
            log,
            tail,
        });
    }
    Ok((found, without_log))
}

/// Writes one group per failure up to `GROUP_LIMIT`, then names the rest.
///
/// Log text is printed between `::stop-commands::` markers so a log line such as
/// `::endgroup::` or `::error::` is shown as text instead of run as a workflow command.
fn render(
    out: &mut impl Write,
    failures: &[Failure],
    without_log: usize,
    token: &str,
) -> std::io::Result<()> {
    for failure in failures.iter().take(GROUP_LIMIT) {
        writeln!(out, "::group::{}", escape_data(&failure.title))?;
        let (source, tail) = tail_of(failure);
        writeln!(out, "{source}")?;
        if let Some(tail) = tail {
            writeln!(out, "::stop-commands::{token}")?;
            writeln!(out, "{tail}")?;
            writeln!(out, "::{token}::")?;
        }
        writeln!(out, "::endgroup::")?;
    }
    let rest = failures.get(GROUP_LIMIT..).unwrap_or_default();
    if !rest.is_empty() {
        writeln!(
            out,
            "{} more failing tests, logs not shown (the first {GROUP_LIMIT} are above):",
            rest.len()
        )?;
        for failure in rest {
            writeln!(out, "  {}", failure.title)?;
        }
    }
    if without_log > 0 {
        writeln!(
            out,
            "{without_log} failing tests left no test.log (failed to build or aborted)"
        )?;
    }
    Ok(())
}

/// A line naming where the tail came from, and the tail itself when there is one.
fn tail_of(failure: &Failure) -> (String, Option<String>) {
    let Some(log) = &failure.log else {
        return (
            "tail kept by dx (log path unknown):".to_owned(),
            failure.tail.clone(),
        );
    };
    match std::fs::read(log) {
        Ok(bytes) => match dx_bep::log_tail(&bytes, TAIL_LINES, TAIL_BYTES) {
            Some(tail) => (
                format!("last {} lines of {}:", tail.lines().count(), log.display()),
                Some(tail),
            ),
            None => (format!("{} is empty", log.display()), None),
        },
        Err(err) if failure.tail.is_some() => (
            format!("cannot read {} ({err}); tail kept by dx:", log.display()),
            failure.tail.clone(),
        ),
        Err(err) => (format!("cannot read {}: {err}", log.display()), None),
    }
}

/// Escapes workflow command data so a title stays on its one command line.
fn escape_data(text: &str) -> String {
    text.replace('%', "%25")
        .replace('\r', "%0D")
        .replace('\n', "%0A")
}

/// A token no test log is expected to contain, to end `::stop-commands::`.
fn stop_token() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or_default();
    format!("test-log-tails-{}-{nanos}", std::process::id())
}

#[cfg(test)]
mod tests {
    use super::{Failure, GROUP_LIMIT, bep_failures, dx_failures, render};
    use std::path::Path;

    fn result_event(label: &str, attempt: u32, status: &str, log: &Path) -> String {
        let uri = dx_path::uri(log).expect("uri");
        format!(
            r#"{{"id":{{"testResult":{{"label":"{label}","run":1,"shard":1,"attempt":{attempt}}}}},"testResult":{{"status":"{status}","testActionOutput":[{{"name":"test.log","uri":"{uri}"}}]}}}}"#
        )
    }

    fn rendered(failures: &[Failure], without_log: usize) -> String {
        let mut out = Vec::new();
        render(&mut out, failures, without_log, "TOKEN").expect("render");
        String::from_utf8(out).expect("utf8")
    }

    #[test]
    fn bep_keeps_the_last_attempt_of_each_failing_test() {
        let dir = dx_test_scratch::scratch("test_log_tails").expect("scratch");
        let log = dir.path().join("test.log");
        std::fs::write(&log, "boom\n").expect("log");
        let stream = [
            result_event("//a:flaky", 1, "FAILED", &log),
            result_event("//a:flaky", 2, "PASSED", &log),
            result_event("//a:broken", 1, "FAILED", &log),
            result_event("//a:broken", 2, "TIMEOUT", &log),
            result_event("//a:fine", 1, "PASSED", &log),
        ]
        .join("\n");
        let bep = dir.path().join("bep.json");
        std::fs::write(&bep, stream).expect("bep");
        let (found, without_log) = bep_failures(&bep, dir.path()).expect("read");
        assert_eq!(without_log, 0);
        assert_eq!(
            found,
            vec![Failure {
                title: "//a:broken TIMEOUT".to_owned(),
                log: Some(log),
                tail: None,
            }]
        );
    }

    #[test]
    fn bep_counts_failures_without_a_log() {
        let dir = dx_test_scratch::scratch("test_log_tails").expect("scratch");
        let bep = dir.path().join("bep.json");
        std::fs::write(
            &bep,
            r#"{"id":{"testResult":{"label":"//a:t","run":1,"shard":1,"attempt":1}},"testResult":{"status":"FAILED"}}"#,
        )
        .expect("bep");
        let (found, without_log) = bep_failures(&bep, dir.path()).expect("read");
        assert!(found.is_empty());
        assert_eq!(without_log, 1);
    }

    #[test]
    fn dx_reads_final_failing_outcomes() {
        let dir = dx_test_scratch::scratch("test_log_tails").expect("scratch");
        let events = dir.path().join("gate.json");
        std::fs::write(
            &events,
            [
                r#"{"event":"test_outcome","target":"//a:t","outcome":"failed","log_path":"/x/test.log","log_tail":"why"}"#,
                r#"{"event":"test_outcome","target":"//a:t","outcome":"failed","final":false,"log_path":"/x/old.log"}"#,
                r#"{"event":"test_outcome","target":"//a:ok","outcome":"passed"}"#,
                r#"{"event":"test_outcome","target":"//a:gone","outcome":"aborted"}"#,
                "not json",
                r#"{"event":"command_finished","exit_code":1}"#,
            ]
            .join("\n"),
        )
        .expect("events");
        let (found, without_log) = dx_failures(&events).expect("read");
        assert_eq!(without_log, 1);
        assert_eq!(
            found,
            vec![Failure {
                title: "//a:t FAILED".to_owned(),
                log: Some("/x/test.log".into()),
                tail: Some("why".to_owned()),
            }]
        );
    }

    #[test]
    fn render_groups_the_tail_and_fences_workflow_commands() {
        let dir = dx_test_scratch::scratch("test_log_tails").expect("scratch");
        let log = dir.path().join("test.log");
        let lines: Vec<String> = (1..=100).map(|n| format!("line {n}")).collect();
        std::fs::write(&log, format!("{}\n::endgroup::\n", lines.join("\n"))).expect("log");
        let text = rendered(
            &[Failure {
                title: "//a:t FAILED".to_owned(),
                log: Some(log),
                tail: None,
            }],
            0,
        );
        assert!(text.starts_with("::group:://a:t FAILED\nlast 80 lines of "));
        assert!(!text.contains("line 21\n"));
        assert!(text.contains("::stop-commands::TOKEN\nline 22\n"));
        assert!(text.ends_with("line 100\n::endgroup::\n::TOKEN::\n::endgroup::\n"));
    }

    #[test]
    fn render_falls_back_to_the_dx_tail_and_caps_the_groups() {
        let missing = Failure {
            title: "//a:t FAILED".to_owned(),
            log: Some("/no/such/test.log".into()),
            tail: Some("kept".to_owned()),
        };
        let text = rendered(&[missing], 0);
        assert!(text.contains("tail kept by dx:\n::stop-commands::TOKEN\nkept\n::TOKEN::\n"));

        let many: Vec<Failure> = (0..GROUP_LIMIT + 2)
            .map(|n| Failure {
                title: format!("//a:t{n} FAILED"),
                log: None,
                tail: Some("x".to_owned()),
            })
            .collect();
        let text = rendered(&many, 3);
        assert_eq!(text.matches("::group::").count(), GROUP_LIMIT);
        assert!(text.contains("2 more failing tests, logs not shown"));
        assert!(text.contains(&format!("  //a:t{} FAILED\n", GROUP_LIMIT + 1)));
        assert!(text.ends_with("3 failing tests left no test.log (failed to build or aborted)\n"));
    }
}
