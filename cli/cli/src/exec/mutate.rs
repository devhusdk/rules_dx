//! `dx mutate`: runs dx-mutate targets for a time budget and reports the surviving mutants.

use std::path::Path;

use serde::Deserialize;

use super::common::*;
use dx_plan::args::grammar::fuzz_seconds;
use dx_plan::args::{Invocation, Profile};
use dx_plan::plan::plan_run;
use dx_plan::resolve::{exclude, resolve, run_label_query_file};

const DEFAULT_SECONDS: u64 = 60;
const RESULT_FILE: &str = "mutants.json";

#[derive(Deserialize)]
struct Survivor {
    file: String,
    line: u32,
    #[serde(default)]
    mutation: String,
}

#[derive(Deserialize)]
struct Summary {
    killed: u64,
    survived: u64,
    #[serde(default)]
    timed_out: u64,
    #[serde(default)]
    survivors: Vec<Survivor>,
}

impl Summary {
    fn total(&self) -> u64 {
        self.killed + self.survived + self.timed_out
    }

    fn survived_percent(&self) -> f64 {
        match self.total() {
            0 => 0.0,
            total => self.survived as f64 * 100.0 / total as f64,
        }
    }
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
        format!(".dx/local/mutate/{name}/")
    } else {
        format!(".dx/local/mutate/{package}/{name}/")
    }
}

fn read_summary(workspace: &Path, dir: &str) -> Result<Summary, String> {
    let path = workspace.join(dir).join(RESULT_FILE);
    let text = std::fs::read_to_string(&path)
        .map_err(|error| format!("cannot read {dir}{RESULT_FILE}: {error}"))?;
    serde_json::from_str(&text).map_err(|error| format!("bad {dir}{RESULT_FILE}: {error}"))
}

pub(crate) fn execute_mutate(invocation: &Invocation, env: Env<'_>) -> i32 {
    let Env {
        workspace,
        runner,
        query_runner,
        out,
        err,
        ..
    } = env;
    let startup = &invocation.bazel_startup_options;
    let seconds = invocation.time.as_deref().map_or(DEFAULT_SECONDS, |raw| {
        fuzz_seconds(raw).unwrap_or(DEFAULT_SECONDS)
    });
    let scope = match resolve(&invocation.targets, workspace, query_runner, startup) {
        Ok(scope) => scope,
        Err(error) => return resolve_failed(invocation.output, out, err, &error),
    };
    let expression = format!("attr(tags, \"dx-mutate\", {})", exclude::query_set(&scope));
    let labels = match run_label_query_file(
        &expression,
        "dx-mutate targets",
        workspace,
        query_runner,
        startup,
    ) {
        Ok(labels) => labels,
        Err(error) => return resolve_failed(invocation.output, out, err, &error),
    };
    if labels.is_empty() {
        let _ = writeln!(err, "dx: no dx-mutate targets in scope");
        return 0;
    }
    let mut failed = false;
    for label in &labels {
        let dir = out_dir(label);
        if let Err(error) = std::fs::create_dir_all(workspace.join(&dir)) {
            let _ = writeln!(err, "dx: cannot create {dir}: {error}");
            return 1;
        }
        let app_args = vec![
            "--time".to_owned(),
            seconds.to_string(),
            "--out".to_owned(),
            dir.clone(),
        ];
        let run = format!("{label}_run");
        let plan = match plan_run(&run, &[], &app_args, Profile::Dev, startup) {
            Ok(plan) => plan,
            Err(error) => return pre_exec(invocation.output, out, err, &error.to_string()),
        };
        if !invocation.quiet {
            let _ = writeln!(err, "Mutating {label} for {seconds}s");
        }
        let exit = match run_bazel_app(invocation, out, err, workspace, runner, &plan.argv, &[]) {
            Ok(exit) => exit,
            Err(code) => return code,
        };
        match (exit.signal, exit.code) {
            (Some(signo), _) => {
                app_signalled_line(err, &run, signo);
                failed = true;
                continue;
            }
            (None, 0) => {}
            (None, code) => {
                let _ = writeln!(err, "dx: mutate_failed: {run} exited {code}");
                failed = true;
                continue;
            }
        }
        let summary = match read_summary(workspace, &dir) {
            Ok(summary) => summary,
            Err(message) => {
                let _ = writeln!(err, "dx: mutate_failed: {label}: {message}");
                failed = true;
                continue;
            }
        };
        let percent = summary.survived_percent();
        let _ = writeln!(
            out,
            "{label}: killed {}, survived {}, timed out {} ({percent:.1}% survived)",
            summary.killed, summary.survived, summary.timed_out
        );
        for survivor in &summary.survivors {
            let _ = writeln!(
                out,
                "  survived {}:{} {}",
                survivor.file, survivor.line, survivor.mutation
            );
        }
        if let Some(limit) = invocation.fail_survived {
            if percent > f64::from(limit) {
                let _ = writeln!(
                    err,
                    "dx: {label}: {percent:.1}% of mutants survived, over the {limit}% limit"
                );
                failed = true;
            }
        }
    }
    i32::from(failed)
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

    const RESULT: &str = r#"{"killed":6,"survived":3,"timed_out":1,
        "survivors":[{"file":"src/lib.rs","line":12,"mutation":"replace + with -"}]}"#;

    fn run_mutate(
        name: &str,
        args: &[&str],
        exit: Option<i32>,
        result: Option<&str>,
    ) -> (i32, String, String) {
        let harness = Harness::new(name);
        harness
            .query
            .outputs
            .borrow_mut()
            .push(labels("//fixture:x\n"));
        if let Some(result) = result {
            harness.write_source(".dx/local/mutate/fixture/x/mutants.json", result);
        }
        let mut words = vec!["mutate", "//fixture:x"];
        words.extend_from_slice(args);
        let inv = invocation(&words);
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let run = harness.probe_streams(&inv, &[exit], &mut out, &mut err);
        (
            run.code,
            String::from_utf8(out).expect("stdout"),
            String::from_utf8(err).expect("stderr"),
        )
    }

    #[test]
    fn out_dirs_follow_the_package_and_name() {
        assert_eq!(out_dir("//pkg:lib"), ".dx/local/mutate/pkg/lib/");
        assert_eq!(out_dir("//:x"), ".dx/local/mutate/x/");
    }

    #[test]
    fn survivors_are_listed_with_file_and_line() {
        let (code, out, err) = run_mutate("mutate-list", &["--time", "1s"], Some(0), Some(RESULT));
        assert_eq!(code, 0, "{err}");
        assert!(
            out.contains("killed 6, survived 3, timed out 1 (30.0% survived)"),
            "{out}"
        );
        assert!(
            out.contains("survived src/lib.rs:12 replace + with -"),
            "{out}"
        );
    }

    #[test]
    fn fail_on_survived_gates_on_the_percent() {
        let (code, _, err) = run_mutate(
            "mutate-gate",
            &["--fail-on=survived:20"],
            Some(0),
            Some(RESULT),
        );
        assert_eq!(code, 1, "{err}");
        assert!(
            err.contains("30.0% of mutants survived, over the 20% limit"),
            "{err}"
        );
        let (code, _, err) = run_mutate(
            "mutate-pass",
            &["--fail-on=survived:30"],
            Some(0),
            Some(RESULT),
        );
        assert_eq!(code, 0, "{err}");
    }

    #[test]
    fn a_runner_error_or_missing_result_exits_1() {
        let (code, _, err) = run_mutate("mutate-tool", &[], Some(3), None);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("mutate_failed"), "{err}");
        let (code, _, err) = run_mutate("mutate-nofile", &[], Some(0), None);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("cannot read"), "{err}");
    }

    #[test]
    fn no_mutate_targets_is_not_an_error() {
        let harness = Harness::new("mutate-none");
        harness.query.outputs.borrow_mut().push(labels(""));
        let inv = invocation(&["mutate", "//fixture:x"]);
        let run = harness.probe_with(&inv, &[]);
        assert_eq!(run.code, 0, "{run:?}");
        assert!(run.argv.is_empty(), "{run:?}");
    }
}
