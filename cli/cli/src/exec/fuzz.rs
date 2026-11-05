//! `dx fuzz`: runs dx-fuzz targets for a time budget and keeps their crashes.

use std::path::Path;

use super::common::*;
use dx_plan::args::grammar::fuzz_seconds;
use dx_plan::args::{Invocation, Profile};
use dx_plan::plan::{plan_run, shell_join};
use dx_plan::resolve::{quote_set, resolve, run_label_query_file};

const DEFAULT_SECONDS: u64 = 60;
const CRASH_EXIT: i32 = 77;

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
        format!(".dx/fuzz/{name}/")
    } else {
        format!(".dx/fuzz/{package}/{name}/")
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
    let startup = &invocation.bazel_startup_options;
    let seconds = invocation.time.as_deref().map_or(DEFAULT_SECONDS, |raw| {
        fuzz_seconds(raw).unwrap_or(DEFAULT_SECONDS)
    });
    let scope = match resolve(&invocation.targets, workspace, query_runner, startup) {
        Ok(scope) => scope,
        Err(error) => return resolve_failed(invocation.output, out, err, &error),
    };
    let expression = format!(
        "attr(tags, \"dx-fuzz\", set({}))",
        quote_set(&scope.targets)
    );
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
    let mut failed = false;
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
        let plan = match plan_run(&run, &[], &app_args, Profile::Dev, startup) {
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
                failed = true;
            }
            (None, 0) => {}
            (None, CRASH_EXIT) => {
                let crash = newest_crash(workspace, &dir).unwrap_or_else(|| dir.clone());
                let repro = shell_join(&[
                    "bazel".to_owned(),
                    "run".to_owned(),
                    run,
                    "--".to_owned(),
                    "--repro".to_owned(),
                    crash,
                ]);
                let _ = writeln!(err, "dx: {label} crashed; reproduce with: {repro}");
                failed = true;
            }
            (None, code) => {
                let _ = writeln!(err, "dx: fuzz_failed: {run} exited {code}");
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

    #[test]
    fn out_dirs_follow_the_package_and_name() {
        assert_eq!(out_dir("//pkg:parser"), ".dx/fuzz/pkg/parser/");
        assert_eq!(out_dir("//a/b:c"), ".dx/fuzz/a/b/c/");
        assert_eq!(out_dir("//:x"), ".dx/fuzz/x/");
    }

    #[test]
    fn a_crash_prints_the_reproduce_line_and_exits_1() {
        let harness = Harness::new("fuzz-crash");
        harness
            .query
            .outputs
            .borrow_mut()
            .push(labels("//fixture:x\n"));
        harness.write_source(".dx/fuzz/fixture/x/crash-abc", "boom");
        let inv = invocation(&["fuzz", "//fixture:x", "--time", "1s"]);
        let mut err = Vec::new();
        let run = harness.probe_streams(&inv, &[Some(77)], &mut Vec::new(), &mut err);
        let err = String::from_utf8(err).expect("stderr");
        assert_eq!(run.code, 1, "{err}");
        assert!(
            err.contains("dx: //fixture:x crashed; reproduce with: bazel run //fixture:x_run -- --repro .dx/fuzz/fixture/x/crash-abc"),
            "{err}"
        );
        let argv = &run.argv[0];
        assert!(argv.contains(&"//fixture:x_run".to_owned()), "{argv:?}");
        for arg in ["--time", "1", "--out", ".dx/fuzz/fixture/x/"] {
            assert!(argv.contains(&arg.to_owned()), "{argv:?}");
        }
    }

    #[test]
    fn a_clean_run_exits_0_and_a_tool_error_exits_1() {
        for (name, exit, want) in [("fuzz-clean", 0, 0), ("fuzz-tool", 3, 1)] {
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
