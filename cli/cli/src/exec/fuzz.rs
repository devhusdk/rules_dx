//! `dx fuzz`: runs dx-fuzz targets for a time budget and keeps their crashes.

use std::io::Write;
use std::path::Path;

use super::common::*;
use dx_output::{FinishedCounts, NoticeEvent, OutputMode, command_finished, notice_value};
use dx_plan::args::grammar::fuzz_seconds;
use dx_plan::args::{BuildMode, Invocation};
use dx_plan::plan::{plan_run, shell_join};
use dx_plan::resolve::{QueryRunner, exclude, resolve, run_label_query_file};
use dx_process::Runner;

const DEFAULT_SECONDS: u64 = 60;
const CRASH_EXIT: i32 = 77;
const CODE_FUZZ_CRASH: &str = "fuzz_crash";

fn runner_label(label: &str) -> String {
    format!("{label}_run")
}

fn out_dir(label: &str) -> String {
    let path = label.trim_start_matches('@').trim_start_matches("//");
    let (package, name) = path.split_once(':').unwrap_or((path, ""));
    let name = if name.is_empty() {
        package.rsplit('/').next().unwrap_or(package)
    } else {
        name
    };
    if package.is_empty() || package == name {
        format!(".dx/local/fuzz/{name}/")
    } else {
        format!(".dx/local/fuzz/{package}/{name}/")
    }
}

fn newest_crash(workspace: &Path, dir: &str) -> Option<String> {
    let entries = std::fs::read_dir(workspace.join(dir)).ok()?;
    let newest = entries
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().starts_with("crash-"))
        .max_by_key(|entry| entry.metadata().and_then(|meta| meta.modified()).ok())?;
    Some(format!("{dir}{}", newest.file_name().to_string_lossy()))
}

pub(crate) fn execute_fuzz(invocation: &Invocation, env: Env<'_>) -> i32 {
    let Env {
        workspace,
        runner,
        query_runner,
        out,
        err,
        ..
    } = env;
    let json = invocation.output == OutputMode::Json;
    if json && let Err(exit) = emit_started(invocation, out) {
        return exit;
    }
    let code = fuzz_targets(invocation, workspace, runner, query_runner, out, err);
    if json && let Err(exit) = emit_event(out, &command_finished(code, &FinishedCounts::default()))
    {
        return exit;
    }
    code
}

fn crash_event(label: &str, runner: &str, crash: &str, repro: &[String]) -> serde_json::Value {
    let notice = NoticeEvent::warning(
        CODE_FUZZ_CRASH,
        format!("{label} crashed; reproduce with: {}", shell_join(repro)),
    );
    let mut event = notice_value(&notice);
    event["target"] = label.into();
    event["runner"] = runner.into();
    event["crash_path"] = crash.into();
    event["repro"] = repro.into();
    event
}

fn fuzz_targets(
    invocation: &Invocation,
    workspace: &Path,
    runner: &dyn Runner,
    query_runner: &dyn QueryRunner,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let startup = &invocation.bazel_startup_options;
    let seconds = invocation.budget.as_deref().map_or(DEFAULT_SECONDS, |raw| {
        fuzz_seconds(raw).unwrap_or(DEFAULT_SECONDS)
    });
    let scope = match resolve(&invocation.targets, workspace, query_runner, startup) {
        Ok(scope) => scope,
        Err(error) => return resolve_failed(invocation.output, out, err, &error),
    };
    let expression = format!("attr(tags, \"dx-fuzz\", {})", exclude::query_set(&scope));
    let labels = match run_label_query_file(
        &expression,
        "dx-fuzz targets",
        workspace,
        query_runner,
        startup,
    ) {
        Ok(labels) => labels,
        Err(error) => return resolve_failed(invocation.output, out, err, &error),
    };
    if labels.is_empty() {
        let _ = writeln!(err, "dx: no dx-fuzz targets in scope");
        return 0;
    }
    let mut status = 0;
    for label in &labels {
        let dir = out_dir(label);
        if let Err(error) = std::fs::create_dir_all(workspace.join(&dir)) {
            let _ = writeln!(err, "dx: cannot create {dir}: {error}");
            return 1;
        }
        let mut app_args = vec![
            "--time".to_owned(),
            seconds.to_string(),
            "--out".to_owned(),
            dir.clone(),
        ];
        if let Some(jobs) = invocation.jobs {
            app_args.extend(["--jobs".to_owned(), jobs.to_string()]);
        }
        let run = runner_label(label);
        let plan = match plan_run(&run, &[], &app_args, BuildMode::Dev, startup) {
            Ok(plan) => plan,
            Err(error) => return pre_exec(invocation.output, out, err, &error.to_string()),
        };
        if !invocation.quiet {
            let _ = writeln!(err, "Fuzzing {label} for {seconds}s");
        }
        let exit = match run_bazel_app(invocation, out, err, workspace, runner, &plan.argv, &[]) {
            Ok(exit) => exit,
            Err(code) => return code,
        };
        match (exit.signal, exit.code) {
            (Some(signo), _) => {
                app_signalled_line(err, &run, signo);
                if status == 0 {
                    status = 128 + signo;
                }
            }
            (None, 0) => {}
            (None, CRASH_EXIT) => {
                let crash = newest_crash(workspace, &dir).unwrap_or_else(|| dir.clone());
                let repro = [
                    "dx".to_owned(),
                    "run".to_owned(),
                    run.clone(),
                    "--".to_owned(),
                    "--repro".to_owned(),
                    crash.clone(),
                ];
                if invocation.output == OutputMode::Json {
                    if let Err(exit) = emit_event(out, &crash_event(label, &run, &crash, &repro)) {
                        return exit;
                    }
                } else {
                    let _ = writeln!(
                        err,
                        "dx: {label} crashed; reproduce with: {}",
                        shell_join(&repro)
                    );
                }
                if status == 0 {
                    status = 1;
                }
            }
            (None, code) => {
                let _ = writeln!(err, "dx: fuzz_failed: {run} exited {code}");
                if status == 0 {
                    status = code;
                }
            }
        }
    }
    status
}

#[cfg(test)]
mod tests {
    use super::super::test_support::*;
    use super::out_dir;
    use dx_plan::resolve::QueryResult;

    fn labels(output: &str) -> QueryResult {
        QueryResult {
            code: Some(0),
            stdout: output.as_bytes().to_vec(),
            stderr: Vec::new(),
        }
    }

    #[test]
    fn out_dirs_follow_the_package_and_name() {
        assert_eq!(out_dir("//pkg:parser"), ".dx/local/fuzz/pkg/parser/");
        assert_eq!(out_dir("//a/b:c"), ".dx/local/fuzz/a/b/c/");
        assert_eq!(out_dir("//:x"), ".dx/local/fuzz/x/");
    }

    #[test]
    fn a_crash_prints_the_reproduce_line_and_exits_1() {
        let harness = Harness::new("fuzz-crash");
        harness
            .query
            .outputs
            .borrow_mut()
            .push(labels("//fixture:x\n"));
        harness.write_source(".dx/local/fuzz/fixture/x/crash-abc", "boom");
        let inv = invocation(&["fuzz", "//fixture:x", "--budget", "1s"]);
        let mut err = Vec::new();
        let run = harness.probe_streams(&inv, &[Some(77)], &mut Vec::new(), &mut err);
        let err = String::from_utf8(err).expect("stderr");
        assert_eq!(run.code, 1, "{err}");
        assert!(
            err.contains("dx: //fixture:x crashed; reproduce with: dx run //fixture:x_run -- --repro .dx/local/fuzz/fixture/x/crash-abc"),
            "{err}"
        );
        let argv = &run.argv[0];
        assert!(argv.contains(&"//fixture:x_run".to_owned()), "{argv:?}");
        for arg in ["--time", "1", "--out", ".dx/local/fuzz/fixture/x/"] {
            assert!(argv.contains(&arg.to_owned()), "{argv:?}");
        }
    }

    #[test]
    fn json_output_reports_each_crash_without_bazel() {
        let harness = Harness::new("fuzz-json");
        harness
            .query
            .outputs
            .borrow_mut()
            .push(labels("//fixture:x\n"));
        harness.write_source(".dx/local/fuzz/fixture/x/crash-abc", "boom");
        let inv = invocation(&["fuzz", "//fixture:x", "--output", "json"]);
        let mut out = Vec::new();
        let mut err = Vec::new();
        let run = harness.probe_streams(&inv, &[Some(77)], &mut out, &mut err);
        assert_eq!(run.code, 1);
        let out = String::from_utf8(out).expect("stdout");
        let events: Vec<serde_json::Value> = out
            .lines()
            .map(|line| serde_json::from_str(line).expect("json"))
            .collect();
        let kinds: Vec<&str> = events
            .iter()
            .map(|e| e["event"].as_str().unwrap())
            .collect();
        assert_eq!(
            kinds,
            ["command_started", "notice", "command_finished"],
            "{out}"
        );
        let crash = &events[1];
        assert_eq!(crash["code"], "fuzz_crash");
        assert_eq!(crash["target"], "//fixture:x");
        assert_eq!(crash["runner"], "//fixture:x_run");
        assert_eq!(crash["crash_path"], ".dx/local/fuzz/fixture/x/crash-abc");
        assert_eq!(
            crash["repro"],
            serde_json::json!([
                "dx",
                "run",
                "//fixture:x_run",
                "--",
                "--repro",
                ".dx/local/fuzz/fixture/x/crash-abc"
            ])
        );
        assert!(!out.contains("bazel"), "{out}");
        assert_eq!(events[2]["exit_code"], 1);
    }

    #[test]
    fn a_clean_run_exits_0_and_a_tool_error_keeps_its_code() {
        for (name, exit, want) in [("fuzz-clean", 0, 0), ("fuzz-tool", 3, 3)] {
            let harness = Harness::new(name);
            harness
                .query
                .outputs
                .borrow_mut()
                .push(labels("//fixture:x\n"));
            let inv = invocation(&["fuzz", "//fixture:x"]);
            let mut err = Vec::new();
            let run = harness.probe_streams(&inv, &[Some(exit)], &mut Vec::new(), &mut err);
            let err = String::from_utf8(err).expect("stderr");
            assert_eq!(run.code, want, "{err}");
            assert!(!err.contains("crashed"), "{err}");
            let argv = &run.argv[0];
            assert!(argv.contains(&"60".to_owned()), "{argv:?}");
        }
    }

    #[test]
    fn no_fuzz_targets_is_not_an_error() {
        let harness = Harness::new("fuzz-none");
        harness.query.outputs.borrow_mut().push(labels(""));
        let inv = invocation(&["fuzz", "//fixture:x"]);
        let run = harness.probe_with(&inv, &[]);
        assert_eq!(run.code, 0, "{run:?}");
        assert!(run.argv.is_empty(), "{run:?}");
    }
}
