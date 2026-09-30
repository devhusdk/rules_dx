use std::path::{Path, PathBuf};
use std::time::Duration;

use dx_adopt::AdoptError;

use crate::args::{Command, Invocation};
use crate::exec::common::check_stdout_write;
use crate::exec::Env;

use super::{operational, pre_exec, summaries_suppressed};

const WATCH_IDLE_TIMEOUT: Duration = Duration::from_secs(60);

/// Runs the wrapped command, then reruns it after every change until interrupted.
pub(crate) fn execute_watch(invocation: &Invocation, env: Env<'_>) -> i32 {
    watch(invocation, env, None, |root| {
        dx_adopt::watch_for_change(root, WATCH_IDLE_TIMEOUT)
    })
}

fn watch(
    invocation: &Invocation,
    env: Env<'_>,
    max_iterations: Option<u32>,
    mut wait_for_change: impl FnMut(&Path) -> Result<Vec<PathBuf>, AdoptError>,
) -> i32 {
    let Env {
        workspace,
        runner,
        query_runner,
        temp_dir,
        pid,
        nonce,
        mut out,
        mut err,
        ci,
    } = env;
    let wrapped = invocation
        .targets
        .first()
        .map(String::as_str)
        .unwrap_or_default();
    let scopes = invocation.targets.get(1..).unwrap_or_default();
    let plan = match dx_adopt::plan_watch(wrapped, ci) {
        Ok(plan) => plan,
        Err(error) => return pre_exec(&mut err, &error.to_string()),
    };
    let command = match Command::parse(wrapped) {
        Some(command) => command,
        None => return pre_exec(&mut err, &format!("not watchable: {wrapped}")),
    };
    let scope = scopes.join(" ");
    let suppressed = summaries_suppressed(invocation);
    if invocation.dry_run {
        if !suppressed {
            if let Err(exit) = check_stdout_write(writeln!(out, "would {plan} scope={scope}")) {
                return exit;
            }
        }
        return 0;
    }
    let mut iteration = invocation.clone();
    iteration.command = command;
    iteration.targets = scopes.to_vec();
    let mut round = 0u32;
    loop {
        round += 1;
        if !suppressed {
            if let Err(exit) =
                check_stdout_write(writeln!(out, "{plan} scope={scope} iteration={round}"))
            {
                return exit;
            }
        }
        let code = crate::exec::execute(
            &iteration,
            Env {
                workspace,
                runner,
                query_runner,
                temp_dir,
                pid,
                nonce,
                out: &mut out,
                err: &mut err,
                ci,
            },
        );
        if max_iterations == Some(round) {
            return code;
        }
        if round == 1 && !suppressed {
            if let Err(exit) = check_stdout_write(writeln!(out, "watching (Ctrl-C to stop)")) {
                return exit;
            }
        }
        loop {
            match wait_for_change(workspace) {
                Ok(paths) if paths.is_empty() => {}
                Ok(paths) => {
                    if !suppressed {
                        if let Err(exit) =
                            check_stdout_write(writeln!(out, "{}", changed_line(&paths)))
                        {
                            return exit;
                        }
                    }
                    break;
                }
                Err(error) => {
                    return operational(
                        invocation,
                        &mut out,
                        &mut err,
                        "watch_failed",
                        &error.to_string(),
                    )
                }
            }
        }
    }
}

fn changed_line(paths: &[PathBuf]) -> String {
    if paths.len() == 1 {
        return format!("changed {}", paths[0].display());
    }
    format!("changed {} (+{} more)", paths[0].display(), paths.len() - 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::args::parse;
    use std::cell::RefCell;
    use std::io::{self, Write};

    fn invocation(words: &[&str]) -> Invocation {
        parse(&words.iter().map(ToString::to_string).collect::<Vec<_>>()).expect("parse")
    }

    struct NullQuery;

    impl crate::resolve::QueryRunner for NullQuery {
        fn run_query(
            &self,
            _argv: &[String],
            _cwd: &Path,
        ) -> io::Result<crate::resolve::QueryResult> {
            Ok(crate::resolve::QueryResult {
                code: Some(0),
                stdout: b"//a:one\n".to_vec(),
                stderr: Vec::new(),
            })
        }
    }

    struct CountingRunner {
        seen: RefCell<Vec<String>>,
    }

    const MINIMAL_TEST_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?><testsuites><testsuite name="s"><testcase name="passes" classname="c" time="0.1"/></testsuite></testsuites>"#;

    fn passed_bep(bep: &str, label: &str) {
        let xml = Path::new(bep).with_file_name("watch-test.xml");
        std::fs::write(&xml, MINIMAL_TEST_XML).expect("test.xml");
        let uri = format!("file://{}", xml.display());
        let lines = [
            serde_json::json!({
                "id": {"namedSet": {"id": "0"}},
                "namedSetOfFiles": {"files": []},
            }),
            serde_json::json!({
                "id": {"targetCompleted": {"label": label}},
                "completed": {
                    "success": true,
                    "outputGroup": [{"name": "dx_results", "fileSets": [{"id": "0"}]}],
                },
            }),
            serde_json::json!({
                "id": {"testResult": {"label": label}},
                "testResult": {
                    "status": "PASSED",
                    "testActionOutput": [{"name": "test.xml", "uri": uri}],
                },
            }),
        ];
        let text = lines
            .iter()
            .map(serde_json::Value::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        std::fs::write(bep, text).unwrap_or_else(|e| panic!("BEP {bep}: {e}"));
    }

    impl dx_process::Runner for CountingRunner {
        fn run(
            &self,
            argv: &[String],
            _cwd: &Path,
            _env: &[(&str, &str)],
        ) -> io::Result<dx_process::ChildStatus> {
            self.seen.borrow_mut().push(argv.join(" "));
            if let Some(bep) = argv
                .iter()
                .find_map(|arg| arg.strip_prefix("--build_event_json_file="))
            {
                passed_bep(bep, "//a:one");
            }
            Ok(dx_process::ChildStatus { code: Some(0) })
        }
    }

    struct Truncated {
        remaining: usize,
    }

    impl Write for Truncated {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if self.remaining == 0 {
                return Err(io::Error::new(io::ErrorKind::BrokenPipe, "broken pipe"));
            }
            let written = self.remaining.min(bytes.len());
            self.remaining -= written;
            Ok(written)
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    struct Harness {
        out: Vec<u8>,
        err: Vec<u8>,
        seen: RefCell<Vec<String>>,
        workspace: PathBuf,
        temp_dir: PathBuf,
        ci: bool,
        _guard: dx_test_scratch::TempDir,
    }

    impl Harness {
        fn new(name: &str) -> Self {
            let scratch = dx_test_scratch::scratch(name);
            let root = scratch.path().to_path_buf();
            let temp_dir = root.join(".tmp");
            std::fs::create_dir_all(&temp_dir).expect("temp dir");
            Self {
                out: Vec::new(),
                err: Vec::new(),
                seen: RefCell::new(Vec::new()),
                workspace: root,
                temp_dir,
                ci: false,
                _guard: scratch,
            }
        }

        fn in_ci(mut self) -> Self {
            self.ci = true;
            self
        }

        fn run(
            &mut self,
            words: &[&str],
            max_iterations: Option<u32>,
            wait: &mut dyn FnMut(&Path) -> Result<Vec<PathBuf>, AdoptError>,
        ) -> i32 {
            let query = NullQuery;
            let runner = CountingRunner {
                seen: RefCell::new(Vec::new()),
            };
            let code = watch(
                &invocation(words),
                Env {
                    workspace: &self.workspace,
                    runner: &runner,
                    query_runner: &query,
                    temp_dir: &self.temp_dir,
                    pid: 1,
                    nonce: 1,
                    out: &mut self.out,
                    err: &mut self.err,
                    ci: self.ci,
                },
                max_iterations,
                |root| wait(root),
            );
            *self.seen.borrow_mut() = runner.seen.borrow().clone();
            code
        }

        fn run_with_truncated_out(&mut self, words: &[&str], max_iterations: Option<u32>) -> i32 {
            let query = NullQuery;
            let runner = CountingRunner {
                seen: RefCell::new(Vec::new()),
            };
            let mut out = Truncated { remaining: 0 };
            let mut err = Vec::new();
            watch(
                &invocation(words),
                Env {
                    workspace: &self.workspace,
                    runner: &runner,
                    query_runner: &query,
                    temp_dir: &self.temp_dir,
                    pid: 1,
                    nonce: 1,
                    out: &mut out,
                    err: &mut err,
                    ci: self.ci,
                },
                max_iterations,
                |_| Ok(Vec::new()),
            )
        }

        fn stdout(&self) -> String {
            String::from_utf8(self.out.clone()).expect("stdout")
        }

        fn stderr(&self) -> String {
            String::from_utf8(self.err.clone()).expect("stderr")
        }
    }

    #[test]
    fn watch_runs_the_wrapped_command_once_without_changes() {
        let mut harness = Harness::new("dx-watch-once-");
        let code = harness.run(
            &["watch", "test", "//..."],
            Some(1),
            &mut |_| Ok(Vec::new()),
        );
        assert_eq!(code, 0, "{}", harness.stderr());
        assert_eq!(harness.seen.borrow().len(), 1, "one wrapped run");
        assert!(
            harness.seen.borrow()[0].contains(" test "),
            "wraps test: {}",
            harness.seen.borrow()[0]
        );
        let stdout = harness.stdout();
        assert_eq!(
            stdout.lines().next(),
            Some("watch:test:debounce=200ms scope=//... iteration=1"),
            "scope excludes the wrapped command: {stdout}"
        );
        assert!(
            stdout.contains("Running test for //..."),
            "the wrapped command keeps its own output: {stdout}"
        );
    }

    #[test]
    fn watch_reruns_the_wrapped_command_after_every_change() {
        let mut harness = Harness::new("dx-watch-rerun-");
        let mut waits = 0;
        let code = harness.run(&["watch", "build", "//cli/..."], None, &mut |_| {
            waits += 1;
            match waits {
                1 | 2 => Ok(vec![
                    PathBuf::from("cli/cli/src/lib.rs"),
                    PathBuf::from("cli/cli/src/main.rs"),
                ]),
                _ => Err(AdoptError::WatchFailed {
                    detail: "stop".to_owned(),
                }),
            }
        });
        assert_eq!(code, 1, "watch failures are operational");
        assert_eq!(harness.seen.borrow().len(), 3, "one run plus two reruns");
        let stdout = harness.stdout();
        assert!(stdout.contains("iteration=1"), "{stdout}");
        assert!(stdout.contains("iteration=3"), "{stdout}");
        assert!(
            stdout.contains("watching (Ctrl-C to stop)"),
            "announced once: {stdout}"
        );
        assert_eq!(
            stdout
                .lines()
                .filter(|line| line.starts_with("changed "))
                .count(),
            2,
            "one line per trigger: {stdout}"
        );
        assert!(
            stdout.contains("changed cli/cli/src/lib.rs (+1 more)"),
            "bursts stay one line: {stdout}"
        );
        assert!(
            harness.stderr().contains("watch_failed"),
            "{}",
            harness.stderr()
        );
    }

    #[test]
    fn watch_keeps_watching_while_nothing_changes() {
        let mut harness = Harness::new("dx-watch-idle-");
        let mut waits = 0;
        let code = harness.run(&["watch", "build", "//..."], None, &mut |_| {
            waits += 1;
            if waits < 3 {
                return Ok(Vec::new());
            }
            Err(AdoptError::WatchSpawn {
                detail: "stop".to_owned(),
            })
        });
        assert_eq!(code, 1);
        assert_eq!(harness.seen.borrow().len(), 1, "idle never reruns");
    }

    #[test]
    fn watch_dry_run_plans_without_running_or_waiting() {
        let mut harness = Harness::new("dx-watch-dry-run-");
        let code = harness.run(&["watch", "--dry-run", "lint", "//..."], None, &mut |_| {
            panic!("dry run never waits for changes")
        });
        assert_eq!(code, 0);
        assert!(harness.seen.borrow().is_empty(), "launches nothing");
        assert_eq!(
            harness.stdout(),
            "would watch:lint:debounce=200ms scope=//...\n"
        );
    }

    #[test]
    fn watch_quiet_drops_summaries_and_keeps_running() {
        let mut harness = Harness::new("dx-watch-quiet-");
        let code = harness.run(&["watch", "--quiet", "build", "//..."], None, &mut |_| {
            Err(AdoptError::WatchSpawn {
                detail: "stop".to_owned(),
            })
        });
        assert_eq!(code, 1);
        assert_eq!(harness.seen.borrow().len(), 1);
        assert!(harness.stdout().is_empty(), "{}", harness.stdout());
    }

    #[test]
    fn watch_refuses_unwatchable_commands_and_ci() {
        for words in [
            vec!["watch", "docs"],
            vec!["watch", "watch"],
            vec!["watch", "//nope"],
        ] {
            let mut harness = Harness::new("dx-watch-refuses-");
            let code = harness.run(&words, Some(1), &mut |_| Ok(Vec::new()));
            assert_eq!(code, 2, "{words:?}");
            assert!(
                harness.stderr().starts_with("dx: not watchable"),
                "{words:?}"
            );
            assert!(harness.seen.borrow().is_empty(), "{words:?}");
        }
        let mut harness = Harness::new("dx-watch-ci-").in_ci();
        let code = harness.run(&["watch", "build", "//..."], Some(1), &mut |_| {
            Ok(Vec::new())
        });
        assert_eq!(code, 2);
        assert!(
            harness.stderr().contains("refuses CI"),
            "{}",
            harness.stderr()
        );
        assert!(harness.seen.borrow().is_empty());
    }

    #[test]
    fn every_watchable_command_name_resolves_to_a_command() {
        for watchable in dx_adopt::WATCHABLE_COMMANDS {
            assert!(
                Command::parse(watchable).is_some(),
                "{watchable} must resolve or the wrap step cannot run it"
            );
        }
    }

    #[test]
    fn watch_broken_pipe_on_the_summary_returns_141() {
        for words in [
            vec!["watch", "build", "//..."],
            vec!["watch", "--dry-run", "build", "//..."],
        ] {
            let mut harness = Harness::new("dx-watch-pipe-");
            assert_eq!(
                harness.run_with_truncated_out(&words, Some(1)),
                141,
                "{words:?}"
            );
        }
    }

    #[test]
    fn watch_quiet_never_writes_a_summary_so_no_pipe_can_break() {
        let mut harness = Harness::new("dx-watch-quiet-pipe-");
        assert_eq!(
            harness.run_with_truncated_out(&["watch", "--quiet", "build", "//..."], Some(1)),
            0
        );
    }
}
