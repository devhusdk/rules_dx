use crate::exec::{print_actions, ActionTally};
use std::cell::RefCell;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

use dx_adopt::AdoptError;
use dx_output::OutputMode;

use crate::exec::common::check_stdout_write;
use crate::exec::Env;
use dx_plan::args::{Command, Invocation};

use super::{operational, pre_exec, summaries_suppressed};

const WATCH_POLL: Duration = Duration::from_millis(100);

/// Runs the wrapped command, then reruns it after every change until interrupted.
pub(crate) fn execute_watch(invocation: &Invocation, env: Env<'_>) -> i32 {
    watch(
        invocation,
        env,
        None,
        &dx_process::interrupted,
        |session, timeout| session.wait(timeout),
    )
}

fn report_paths(iteration: &Invocation, workspace: &Path) -> Vec<PathBuf> {
    iteration
        .reports
        .iter()
        .filter(|report| report.destination != "-")
        .map(|report| dx_plan::reports::resolve_destination(workspace, &report.destination))
        .collect()
}

/// Where watch writes its own progress lines.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Progress {
    Silent,
    Stdout,
    Stderr,
}

impl Progress {
    fn of(invocation: &Invocation) -> Self {
        if summaries_suppressed(invocation) {
            return Progress::Silent;
        }
        let text = matches!(invocation.output, OutputMode::Text { .. });
        let report_on_stdout = invocation
            .reports
            .iter()
            .any(|report| report.destination == "-");
        if text && !report_on_stdout {
            Progress::Stdout
        } else {
            Progress::Stderr
        }
    }

    fn say(self, out: &mut dyn Write, err: &mut dyn Write, line: &str) -> Result<(), i32> {
        match self {
            Progress::Silent => Ok(()),
            Progress::Stdout => check_stdout_write(writeln!(out, "{line}")),
            Progress::Stderr => {
                let _ = writeln!(err, "{line}");
                Ok(())
            }
        }
    }
}

fn refuse(invocation: &Invocation, out: &mut dyn Write, err: &mut dyn Write, message: &str) -> i32 {
    pre_exec(invocation.output, out, err, message)
}

fn stopped(progress: Progress, out: &mut dyn Write, err: &mut dyn Write, signo: i32) -> i32 {
    if let Err(exit) = progress.say(out, err, "stopped") {
        return exit;
    }
    dx_process::interrupted_code(signo)
}

fn watch(
    invocation: &Invocation,
    env: Env<'_>,
    max_iterations: Option<u32>,
    stop: &dyn Fn() -> Option<i32>,
    wait_for_change: impl FnMut(
        &dx_adopt::watch::WatchSession,
        Duration,
    ) -> Result<Vec<PathBuf>, AdoptError>,
) -> i32 {
    let Env {
        workspace,
        runner,
        query_runner,
        temp_dir,
        pid,
        nonce,
        out,
        mut err,
        ci,
        audit_today,
        actions: _,
        proc_root,
        bump_commit_json,
    } = env;
    let wrapped = invocation
        .targets
        .first()
        .map(String::as_str)
        .unwrap_or_default();
    let plan = match dx_adopt::plan_watch(wrapped, ci) {
        Ok(plan) => plan,
        Err(error) => return refuse(invocation, out, err, &error.to_string()),
    };
    let progress = Progress::of(invocation);
    let iteration = match build_iteration(invocation) {
        Ok(iteration) => iteration,
        Err(message) => return refuse(invocation, out, err, &message),
    };
    let scope = scope_label(&iteration);
    let session =
        match dx_adopt::watch::WatchSession::start(workspace, &report_paths(&iteration, workspace))
        {
            Ok(session) => session,
            Err(error) => {
                return operational(invocation, out, err, "watch_failed", &error.to_string());
            }
        };
    let wait = RefCell::new(wait_for_change);
    let mut round = 0u32;
    loop {
        if let Some(signo) = stop() {
            return stopped(progress, out, err, signo);
        }
        round += 1;
        if let Err(exit) = progress.say(
            &mut *out,
            &mut *err,
            &format!("{plan} scope={scope} iteration={round}"),
        ) {
            return exit;
        }
        let pending = RefCell::new(None);
        let withdraw = || {
            if pending.borrow().is_some() {
                return true;
            }
            match (wait.borrow_mut())(&session, Duration::ZERO) {
                Ok(paths) if paths.is_empty() => false,
                change => {
                    *pending.borrow_mut() = Some(change);
                    true
                }
            }
        };
        let mut notices = Vec::new();
        let mut phase = dx_plan::slots::Phase::new(iteration.command);
        if let Err(signo) = phase.need(
            dx_plan::slots::needs_slot(iteration.command),
            &mut *err,
            &mut notices,
        ) {
            return stopped(progress, out, err, signo);
        }
        let held = phase.slot().map(|slot| slot.runner(runner));
        let mut iteration_out = dx_plan::slots::AfterFirstLine::new(&mut *out, notices);
        let tally = ActionTally::default();
        let code = crate::exec::execute_until(
            &iteration,
            Env {
                workspace,
                runner: held
                    .as_ref()
                    .map_or(runner, |held| held as &dyn dx_process::Runner),
                query_runner,
                temp_dir,
                pid,
                nonce,
                out: &mut iteration_out,
                err: &mut err,
                ci,
                audit_today,
                actions: &tally,
                proc_root,
                bump_commit_json,
            },
            &withdraw,
        );
        drop(phase);
        print_actions(iteration.output, &tally, &mut *err);
        if max_iterations == Some(round) {
            return code;
        }
        if let Some(signo) = stop() {
            return stopped(progress, out, err, signo);
        }
        if round == 1 {
            if let Err(exit) = progress.say(&mut *out, &mut *err, "watching (Ctrl-C to stop)") {
                return exit;
            }
        }
        let mut change = pending.into_inner();
        loop {
            let paths = match change.take() {
                Some(change) => change,
                None => {
                    if let Some(signo) = stop() {
                        return stopped(progress, out, err, signo);
                    }
                    (wait.borrow_mut())(&session, WATCH_POLL)
                }
            };
            match paths {
                Ok(paths) if paths.is_empty() => {}
                Ok(paths) => {
                    if let Err(exit) = progress.say(&mut *out, &mut *err, &changed_line(&paths)) {
                        return exit;
                    }
                    break;
                }
                Err(error) => {
                    return operational(invocation, out, err, "watch_failed", &error.to_string())
                }
            }
        }
    }
}

/// Builds the invocation one watch iteration executes: the wrapped command with its scopes and flags.
fn build_iteration(invocation: &Invocation) -> Result<Invocation, String> {
    let wrapped = invocation
        .targets
        .first()
        .map(String::as_str)
        .unwrap_or_default();
    let command = match Command::parse(wrapped) {
        Some(command) => command,
        None => return Err(format!("not watchable: {wrapped}")),
    };
    let mut iteration = invocation.clone();
    iteration.command = command;
    iteration.targets = invocation.targets.get(1..).unwrap_or_default().to_vec();
    Ok(iteration)
}

/// Names what each iteration covers: the given scopes, `--affected`, or `//...`.
fn scope_label(iteration: &Invocation) -> String {
    if iteration.affected {
        return "--affected".to_owned();
    }
    if iteration.targets.is_empty() {
        return "//...".to_owned();
    }
    iteration.targets.join(" ")
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
    use crate::adopt::test_support::{
        event_kinds, events_of_kind, invocation, json_events, NullQuery, Truncated,
    };
    use dx_plan::args::OperationMode;
    use std::cell::RefCell;
    use std::io;
    use std::path::Path;
    use std::time::Duration;

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
            serde_json::json!({"id": {"buildMetrics": {}}, "lastMessage": true}),
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
            Ok(dx_process::ChildStatus::exited(0))
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
            wait: &mut dyn FnMut(
                &dx_adopt::watch::WatchSession,
            ) -> Result<Vec<PathBuf>, AdoptError>,
        ) -> i32 {
            self.run_until(words, max_iterations, &|| None, wait)
        }

        fn run_until(
            &mut self,
            words: &[&str],
            max_iterations: Option<u32>,
            stop: &dyn Fn() -> Option<i32>,
            wait: &mut dyn FnMut(
                &dx_adopt::watch::WatchSession,
            ) -> Result<Vec<PathBuf>, AdoptError>,
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
                    audit_today: None,
                    actions: &ActionTally::default(),
                    proc_root: Path::new("/proc"),
                    bump_commit_json: None,
                },
                max_iterations,
                stop,
                |session, _| wait(session),
            );
            *self.seen.borrow_mut() = runner.seen.borrow().clone();
            code
        }

        fn run_with_truncated_out(&mut self, words: &[&str], max_iterations: Option<u32>) -> i32 {
            let query = NullQuery;
            let runner = CountingRunner {
                seen: RefCell::new(Vec::new()),
            };
            let mut out = Truncated::after_bytes(0);
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
                    audit_today: None,
                    actions: &ActionTally::default(),
                    proc_root: Path::new("/proc"),
                    bump_commit_json: None,
                },
                max_iterations,
                &|| None,
                |_, _| Ok(Vec::new()),
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
    fn watch_preserves_the_wrapped_apply_mode() {
        for (words, verb, launches) in [
            (vec!["watch", "run", "//app:bin"], "run", 2),
            (vec!["watch", "build", "//app:bin"], "build", 1),
        ] {
            let mut harness = Harness::new("dx-watch-apply-");
            let code = harness.run(&words, Some(1), &mut |_| Ok(Vec::new()));
            assert_eq!(code, 0, "{words:?}: {}", harness.stderr());
            let seen = harness.seen.borrow();
            assert_eq!(seen.len(), launches, "{words:?}: {seen:?}");
            assert!(
                seen[0].contains(" build "),
                "{words:?} builds first: {seen:?}"
            );
            assert!(
                seen[launches - 1].contains(&format!(" {verb} ")),
                "{words:?} must reach bazel {verb}: {seen:?}"
            );
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
            vec!["watch", "build", "//..."],
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

    #[test]
    fn watch_wraps_a_validated_quality_request() {
        let wrapped = invocation(&["watch", "lint", "//..."]);
        let mut iteration = wrapped.clone();
        iteration.command = Command::Lint;
        iteration.targets = vec!["//...".to_owned()];
        let request = iteration.quality_request().expect("validated request");
        assert_eq!(request.command, Command::Lint);
        assert_eq!(request.targets, vec!["//...".to_owned()]);
    }

    #[test]
    fn watch_iteration_writing_its_report_does_not_queue_another() {
        let scratch = dx_test_scratch::scratch("dx-watch-report-");
        let workspace = scratch.path().to_path_buf();
        let temp_scratch = dx_test_scratch::scratch("dx-watch-report-tmp-");
        let temp_dir = temp_scratch.path().to_path_buf();
        std::fs::create_dir_all(workspace.join("src")).expect("src dir");
        let query = NullQuery;
        let runner = CountingRunner {
            seen: RefCell::new(Vec::new()),
        };
        let mut out = Vec::new();
        let mut err = Vec::new();
        let first = RefCell::new(None);
        let code = watch(
            &invocation(&["watch", "test", "//a:one", "--report", "junit=out.xml"]),
            Env {
                workspace: &workspace,
                runner: &runner,
                query_runner: &query,
                temp_dir: &temp_dir,
                pid: 1,
                nonce: 1,
                out: &mut out,
                err: &mut err,
                ci: false,
                audit_today: None,
                actions: &ActionTally::default(),
                proc_root: Path::new("/proc"),
                bump_commit_json: None,
            },
            Some(2),
            &|| None,
            |session, _| {
                *first.borrow_mut() = Some(session.wait(Duration::from_secs(2))?);
                std::fs::write(workspace.join("src/main.rs"), "fn main() {}").expect("write src");
                session.wait(Duration::from_secs(10))
            },
        );
        assert_eq!(code, 0, "{}", String::from_utf8_lossy(&err));
        assert_eq!(first.borrow().as_deref(), Some(&[][..]));
        assert!(workspace.join("out.xml").exists());
        let stdout = String::from_utf8(out).expect("stdout");
        assert!(
            !stdout
                .lines()
                .any(|line| line.starts_with("changed ") && line.contains("out.xml")),
            "{stdout}"
        );
        assert!(stdout.contains("main.rs"), "{stdout}");
    }

    #[test]
    fn watch_reruns_when_a_change_lands_during_execution() {
        struct WritingRunner {
            workspace: PathBuf,
            seen: RefCell<Vec<String>>,
        }

        impl dx_process::Runner for WritingRunner {
            fn run(
                &self,
                argv: &[String],
                _cwd: &Path,
                _env: &[(&str, &str)],
            ) -> io::Result<dx_process::ChildStatus> {
                self.seen.borrow_mut().push(argv.join(" "));
                std::fs::write(self.workspace.join("trigger.txt"), "edited")
                    .expect("trigger write");
                Ok(dx_process::ChildStatus::exited(0))
            }
        }

        let scratch = dx_test_scratch::scratch("dx-watch-during-run-");
        let workspace = scratch.path().to_path_buf();
        let temp_scratch = dx_test_scratch::scratch("dx-watch-during-run-tmp-");
        let temp_dir = temp_scratch.path().to_path_buf();
        let query = NullQuery;
        let runner = WritingRunner {
            workspace: workspace.clone(),
            seen: RefCell::new(Vec::new()),
        };
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = watch(
            &invocation(&["watch", "build", "//..."]),
            Env {
                workspace: &workspace,
                runner: &runner,
                query_runner: &query,
                temp_dir: &temp_dir,
                pid: 1,
                nonce: 1,
                out: &mut out,
                err: &mut err,
                ci: false,
                audit_today: None,
                actions: &ActionTally::default(),
                proc_root: Path::new("/proc"),
                bump_commit_json: None,
            },
            Some(2),
            &|| None,
            |session, _| session.wait(Duration::from_secs(10)),
        );
        assert_eq!(code, 0, "rerun succeeds: {}", String::from_utf8_lossy(&err));
        assert_eq!(
            runner.seen.borrow().len(),
            2,
            "the mid-run edit must cause exactly one rerun"
        );
        let stdout = String::from_utf8(out).expect("stdout");
        assert!(stdout.contains("iteration=2"), "{stdout}");
        assert!(
            stdout
                .lines()
                .any(|line| line.starts_with("changed ") && line.contains("trigger.txt")),
            "{stdout}"
        );
    }

    #[test]
    fn watch_never_upgrades_a_wrapped_check_to_apply() {
        let check = build_iteration(&invocation(&["watch", "fix", "//..."])).expect("wrapped fix");
        assert_eq!(check.command, Command::Fix);
        assert_eq!(check.targets, vec!["//...".to_owned()]);
        assert_eq!(check.operation(), OperationMode::Check);
        assert!(!check.applies());
        let lint = build_iteration(&invocation(&["watch", "lint", "//..."])).expect("wrapped lint");
        assert_eq!(
            lint.quality_request().expect("quality").mode,
            OperationMode::Check
        );
        let applied =
            build_iteration(&invocation(&["watch", "--apply", "fix", "//..."])).expect("apply fix");
        assert_eq!(applied.operation(), OperationMode::Apply);
        assert!(applied.applies());
    }

    #[test]
    fn watch_forwards_profiles_only_to_commands_that_take_them() {
        let debug =
            build_iteration(&invocation(&["watch", "--debug", "build", "//..."])).expect("debug");
        assert!(debug.debug);
        assert!(!debug.release);
        assert_eq!(debug.targets, vec!["//...".to_owned()]);
        let release = build_iteration(&invocation(&[
            "watch",
            "--release",
            "run",
            "//app:bin",
            "--",
            "--hi",
        ]))
        .expect("release");
        assert!(release.release);
        assert_eq!(
            release.app_args(),
            ["--hi".to_owned()],
            "app args ride the cloned invocation"
        );
        assert!(release.raw_bazel_options().is_empty());
        assert!(dx_plan::args::parse(&["watch", "--debug", "lint", "//..."]).is_err());
    }

    #[test]
    fn watch_parses_wrapped_options_like_the_wrapped_command() {
        let got = invocation(&["watch", "build", "//...", "--", "--jobs=4"]);
        assert_eq!(got.raw_bazel_options(), vec!["--jobs=4".to_owned()]);
        let got = invocation(&["watch", "--apply", "fix", "//app:bin"]);
        assert!(got.apply);
        assert!(dx_plan::args::parse(&["watch", "--apply", "run", "//app:bin"]).is_err());
        let got = invocation(&["watch", "lint", "--fail-on=error", "--report=sarif=a.sarif"]);
        assert_eq!(got.fail_on, dx_output::Threshold::Error);
        assert_eq!(got.reports.len(), 1);
        let got = invocation(&["watch", "-v", "test", "//..."]);
        assert!(got.verbose);
        assert!(dx_plan::args::parse(&["watch", "--check", "build"]).is_err());
        assert!(dx_plan::args::parse(&["watch", "--fail-on=error", "build"]).is_err());
        assert!(dx_plan::args::parse(&["watch", "--report=failures=a.json", "build"]).is_ok());
    }

    #[test]
    fn watch_iteration_carries_wrapped_flags_and_the_dash_payload() {
        let iteration = build_iteration(&invocation(&[
            "watch",
            "lint",
            "--fail-on=error",
            "--report=sarif=out.sarif",
            "//a:one",
            "--",
            "--jobs=4",
            "--apply",
        ]))
        .expect("lint iteration");
        assert_eq!(iteration.command, Command::Lint);
        assert_eq!(iteration.fail_on, dx_output::Threshold::Error);
        assert_eq!(iteration.reports.len(), 1);
        assert_eq!(iteration.targets, vec!["//a:one".to_owned()]);
        assert_eq!(
            iteration.raw_bazel_options(),
            vec!["--jobs=4".to_owned(), "--apply".to_owned()]
        );
        assert!(!iteration.apply);
    }

    #[test]
    fn watch_iteration_carries_include_edits_and_affected() {
        let iteration = build_iteration(&invocation(&[
            "watch",
            "lint",
            "--include-edits",
            "--output=json",
        ]))
        .expect("lint iteration");
        assert_eq!(iteration.command, Command::Lint);
        assert!(iteration.include_edits);
        assert_eq!(iteration.output, OutputMode::Json);

        let iteration =
            build_iteration(&invocation(&["watch", "test", "--affected"])).expect("test iteration");
        assert_eq!(iteration.command, Command::Test);
        assert!(iteration.affected);
        assert!(iteration.targets.is_empty());
    }

    #[test]
    fn watch_scope_label_names_affected_bare_and_explicit_scopes() {
        let label = |words: &[&str]| scope_label(&build_iteration(&invocation(words)).unwrap());
        assert_eq!(label(&["watch", "test", "--affected"]), "--affected");
        assert_eq!(label(&["watch", "build"]), "//...");
        assert_eq!(
            label(&["watch", "build", "//a:one", "//b/..."]),
            "//a:one //b/..."
        );
    }

    #[test]
    fn watch_reaches_the_loop_with_affected() {
        let mut harness = Harness::new("dx-watch-affected-");
        harness.run(&["watch", "test", "--affected"], Some(1), &mut |_| {
            Ok(Vec::new())
        });
        let stdout = harness.stdout();
        assert_eq!(
            stdout.lines().next(),
            Some("watch:test:debounce=200ms scope=--affected iteration=1"),
            "{stdout}\n{}",
            harness.stderr()
        );
        assert!(
            !harness.stderr().contains("not watchable"),
            "{}",
            harness.stderr()
        );
        assert!(!stdout.contains("not watchable"), "{stdout}");
    }

    #[test]
    fn watch_reports_a_session_start_failure_without_running() {
        let missing = PathBuf::from("/tmp/dx-watch-missing-root-does-not-exist");
        let temp_dir = missing.join(".tmp");
        let query = NullQuery;
        let runner = CountingRunner {
            seen: RefCell::new(Vec::new()),
        };
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = watch(
            &invocation(&["watch", "build", "//..."]),
            Env {
                workspace: &missing,
                runner: &runner,
                query_runner: &query,
                temp_dir: &temp_dir,
                pid: 1,
                nonce: 1,
                out: &mut out,
                err: &mut err,
                ci: false,
                audit_today: None,
                actions: &ActionTally::default(),
                proc_root: Path::new("/proc"),
                bump_commit_json: None,
            },
            Some(1),
            &|| None,
            |_, _| Ok(Vec::new()),
        );
        assert_eq!(code, 1);
        assert!(runner.seen.borrow().is_empty(), "nothing runs");
        assert!(
            String::from_utf8(err)
                .expect("stderr")
                .contains("watch_failed"),
            "{}",
            String::from_utf8(out).expect("stdout")
        );
    }

    fn stop_after(calls: u32, signo: i32) -> impl Fn() -> Option<i32> {
        let seen = std::cell::Cell::new(0u32);
        move || {
            seen.set(seen.get() + 1);
            (seen.get() > calls).then_some(signo)
        }
    }

    #[test]
    fn watch_stops_after_the_running_iteration_when_interrupted() {
        let mut harness = Harness::new("dx-watch-stop-running-");
        let code = harness.run_until(
            &["watch", "build", "//..."],
            None,
            &stop_after(1, 2),
            &mut |_| panic!("no wait after the signal"),
        );
        assert_eq!(code, 130, "{}", harness.stderr());
        assert_eq!(harness.seen.borrow().len(), 1, "no new iteration starts");
        let stdout = harness.stdout();
        assert!(stdout.ends_with("stopped\n"), "{stdout}");
        assert!(!stdout.contains("watching"), "{stdout}");
    }

    #[test]
    fn watch_stops_while_idle_when_terminated() {
        let mut harness = Harness::new("dx-watch-stop-idle-");
        let mut waits = 0;
        let code = harness.run_until(
            &["watch", "build", "//..."],
            None,
            &stop_after(4, 15),
            &mut |_| {
                waits += 1;
                Ok(Vec::new())
            },
        );
        assert_eq!(code, 143);
        assert_eq!(waits, 2, "polls until the signal lands");
        assert_eq!(harness.seen.borrow().len(), 1);
        assert!(harness
            .stdout()
            .contains("watching (Ctrl-C to stop)\nstopped\n"));
    }

    #[test]
    fn watch_stops_before_the_first_iteration_when_already_interrupted() {
        let mut harness = Harness::new("dx-watch-stop-early-");
        let code = harness.run_until(
            &["watch", "--quiet", "build", "//..."],
            None,
            &|| Some(2),
            &mut |_| panic!("no wait"),
        );
        assert_eq!(code, 130);
        assert!(harness.seen.borrow().is_empty(), "nothing launches");
        assert!(harness.stdout().is_empty(), "{}", harness.stdout());
    }

    #[test]
    fn watch_json_leaves_stdout_to_each_iteration_event_stream() {
        for words in [
            vec!["watch", "build", "//a:one", "--output=json"],
            vec!["watch", "build", "//a:one", "-o", "json"],
        ] {
            let mut harness = Harness::new("dx-watch-json-");
            let mut waits = 0;
            let code = harness.run(&words, None, &mut |_| {
                waits += 1;
                match waits {
                    1 => Ok(vec![PathBuf::from("/ws/src/lib.rs")]),
                    _ => Err(AdoptError::WatchFailed {
                        detail: "stop".to_owned(),
                    }),
                }
            });
            assert_eq!(code, 1, "{words:?}");
            let stdout = harness.stdout();
            let events = json_events(&stdout);
            let kinds = event_kinds(&events);
            assert_eq!(
                kinds
                    .iter()
                    .filter(|kind| **kind == "command_started")
                    .count(),
                2,
                "{words:?}: {stdout}"
            );
            let finished = events_of_kind(&events, "command_finished");
            assert_eq!(
                finished.len(),
                3,
                "two iterations plus the failure: {stdout}"
            );
            assert_eq!(finished[0]["exit_code"], 0, "{stdout}");
            assert_eq!(finished[2]["exit_code"], 1, "{stdout}");
            assert_eq!(
                events_of_kind(&events, "error")
                    .last()
                    .map(|event| event["code"].clone()),
                Some(serde_json::json!("watch_failed")),
                "{stdout}"
            );
            let stderr = harness.stderr();
            assert!(
                stderr.contains("watch:build:debounce=200ms scope=//a:one iteration=1"),
                "{stderr}"
            );
            assert!(stderr.contains("changed /ws/src/lib.rs"), "{stderr}");
            assert!(stderr.contains("iteration=2"), "{stderr}");
        }
    }

    #[test]
    fn watch_json_refusals_end_in_a_terminal_event() {
        let mut harness = Harness::new("dx-watch-json-refuse-");
        let code = harness.run(&["watch", "docs", "--output=json"], Some(1), &mut |_| {
            Ok(Vec::new())
        });
        assert_eq!(code, 2);
        let events = json_events(&harness.stdout());
        assert_eq!(event_kinds(&events), ["error", "command_finished"]);
        assert_eq!(events[0]["code"], "invalid_arguments");
        assert_eq!(events[1]["exit_code"], 2);
        let mut harness = Harness::new("dx-watch-json-ci-").in_ci();
        let code = harness.run(&["watch", "build", "--output=json"], Some(1), &mut |_| {
            Ok(Vec::new())
        });
        assert_eq!(code, 2);
        let events = json_events(&harness.stdout());
        assert_eq!(event_kinds(&events), ["error", "command_finished"]);
        assert!(harness.seen.borrow().is_empty());
    }

    #[test]
    fn watch_progress_leaves_stdout_to_machine_output() {
        for (words, want) in [
            (vec!["watch", "build"], Progress::Stdout),
            (vec!["watch", "--quiet", "build"], Progress::Silent),
            (vec!["watch", "build", "--output=json"], Progress::Stderr),
            (vec!["watch", "lint", "--output=diff"], Progress::Stderr),
            (vec!["watch", "lint", "-o", "diff"], Progress::Stderr),
            (vec!["watch", "test", "--report=junit=-"], Progress::Stderr),
            (
                vec!["watch", "test", "--report=junit=out.xml"],
                Progress::Stdout,
            ),
        ] {
            assert!(Progress::of(&invocation(&words)) == want, "{words:?}");
        }
    }

    #[test]
    fn watch_diff_writes_each_iteration_patch_and_nothing_else_on_stdout() {
        use crate::exec::test_support::Harness as ExecHarness;
        let mut harness = ExecHarness::new("watch-diff");
        harness.write_source("src/a.py", "x = 1\n");
        harness.results.insert(
            "//test:corpus".to_owned(),
            harness.valid_result(
                vec![ExecHarness::diagnostic("unused", true)],
                vec![harness.replacement(b"y")],
            ),
        );
        let runner = harness.runner();
        let mut out = Vec::new();
        let mut err = Vec::new();
        let mut waits = 0;
        let code = watch(
            &invocation(&["watch", "lint", "--output=diff"]),
            harness.env(&runner, &mut out, &mut err),
            None,
            &|| None,
            |_, _| {
                waits += 1;
                match waits {
                    1 => Ok(vec![PathBuf::from("/ws/src/a.py")]),
                    _ => Err(AdoptError::WatchFailed {
                        detail: "stop".to_owned(),
                    }),
                }
            },
        );
        assert_eq!(code, 1);
        let stdout = String::from_utf8(out).expect("stdout");
        let stderr = String::from_utf8(err).expect("stderr");
        assert_eq!(
            stdout.matches("--- a/src/a.py").count(),
            2,
            "one patch per iteration: {stdout}\n{stderr}"
        );
        assert!(!stdout.contains("watch:"), "{stdout}");
        assert!(!stdout.contains("changed "), "{stdout}");
        assert!(stderr.contains("watch:lint:debounce=200ms"), "{stderr}");
        assert!(stderr.contains("iteration=2"), "{stderr}");
        assert_eq!(
            std::fs::read(harness.workspace.join("src/a.py")).expect("source"),
            b"x = 1\n",
            "a diff watch never writes"
        );
    }

    struct AppRunner {
        events: RefCell<Vec<String>>,
        running: std::cell::Cell<bool>,
    }

    impl AppRunner {
        fn new() -> Self {
            Self {
                events: RefCell::new(Vec::new()),
                running: std::cell::Cell::new(false),
            }
        }
    }

    impl dx_process::Runner for AppRunner {
        fn run(
            &self,
            argv: &[String],
            _cwd: &Path,
            _env: &[(&str, &str)],
        ) -> io::Result<dx_process::ChildStatus> {
            self.events
                .borrow_mut()
                .push(format!("exit {}", argv.join(" ")));
            Ok(dx_process::ChildStatus::exited(0))
        }

        fn run_withdrawable(
            &self,
            argv: &[String],
            _cwd: &Path,
            _env: &[(&str, &str)],
            withdraw: &dyn Fn() -> bool,
        ) -> io::Result<Option<dx_process::ChildStatus>> {
            if argv.iter().any(|arg| arg == "build") {
                self.events
                    .borrow_mut()
                    .push(format!("build {}", argv.join(" ")));
                return Ok(Some(dx_process::ChildStatus::exited(0)));
            }
            assert!(!self.running.get(), "app instances overlap");
            self.running.set(true);
            self.events
                .borrow_mut()
                .push(format!("launch {}", argv.join(" ")));
            for _ in 0..50 {
                if withdraw() {
                    self.events.borrow_mut().push("withdrawn".to_owned());
                    self.running.set(false);
                    return Ok(None);
                }
            }
            self.events.borrow_mut().push("exited".to_owned());
            self.running.set(false);
            Ok(Some(dx_process::ChildStatus::exited(0)))
        }
    }

    fn run_app_watch(
        words: &[&str],
        runner: &AppRunner,
        wait: impl FnMut(&dx_adopt::watch::WatchSession, Duration) -> Result<Vec<PathBuf>, AdoptError>,
    ) -> (i32, String, String) {
        let scratch = dx_test_scratch::scratch("dx-watch-run-");
        let workspace = scratch.path().to_path_buf();
        let temp_dir = workspace.join(".tmp");
        std::fs::create_dir_all(&temp_dir).expect("temp dir");
        let query = NullQuery;
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = watch(
            &invocation(words),
            Env {
                workspace: &workspace,
                runner,
                query_runner: &query,
                temp_dir: &temp_dir,
                pid: 1,
                nonce: 1,
                out: &mut out,
                err: &mut err,
                ci: false,
                audit_today: None,
                actions: &ActionTally::default(),
                proc_root: Path::new("/proc"),
                bump_commit_json: None,
            },
            None,
            &|| None,
            wait,
        );
        (
            code,
            String::from_utf8(out).expect("stdout"),
            String::from_utf8(err).expect("stderr"),
        )
    }

    #[test]
    fn watch_run_stops_the_running_app_before_launching_its_replacement() {
        let runner = AppRunner::new();
        let mut polls = 0;
        let (code, stdout, stderr) = run_app_watch(
            &["watch", "run", "//app:bin", "--", "--help"],
            &runner,
            |_, timeout| {
                assert_eq!(
                    timeout,
                    Duration::ZERO,
                    "a running app is polled, not awaited"
                );
                polls += 1;
                match polls {
                    3 => Ok(vec![PathBuf::from("/ws/src/main.rs")]),
                    6 => Err(AdoptError::WatchFailed {
                        detail: "stop".to_owned(),
                    }),
                    _ => Ok(Vec::new()),
                }
            },
        );
        assert_eq!(code, 1, "{stderr}");
        assert!(stderr.contains("watch_failed"), "{stderr}");
        let events = runner.events.borrow();
        assert_eq!(
            events
                .iter()
                .map(|event| event.split(' ').next().unwrap_or_default())
                .collect::<Vec<_>>(),
            [
                "build",
                "launch",
                "withdrawn",
                "build",
                "launch",
                "withdrawn"
            ],
            "{events:?}"
        );
        let settings = dx_plan::plan::BAZEL_SETTINGS.join(" ");
        assert!(
            events[1].ends_with(&format!(
                " run --config=dx_dev {settings} //app:bin -- --help"
            )),
            "the -- payload reaches the app unchanged: {}",
            events[1]
        );
        assert!(!events[0].contains("--help"), "{}", events[0]);
        assert_eq!(events[1], events[4], "the replacement runs the same argv");
        let lines: Vec<&str> = stdout.lines().collect();
        assert_eq!(
            lines,
            [
                "watch:run:debounce=200ms scope=//app:bin iteration=1",
                "watching (Ctrl-C to stop)",
                "changed /ws/src/main.rs",
                "watch:run:debounce=200ms scope=//app:bin iteration=2",
            ],
            "{stdout}"
        );
    }

    #[test]
    fn watch_run_waits_for_a_change_after_the_app_exits_on_its_own() {
        let runner = AppRunner::new();
        let mut calls = Vec::new();
        let (code, stdout, _) =
            run_app_watch(&["watch", "run", "//app:bin"], &runner, |_, timeout| {
                calls.push(timeout);
                if timeout == Duration::ZERO {
                    return Ok(Vec::new());
                }
                Err(AdoptError::WatchFailed {
                    detail: "stop".to_owned(),
                })
            });
        assert_eq!(code, 1);
        assert_eq!(
            runner.events.borrow().last().map(String::as_str),
            Some("exited")
        );
        assert_eq!(calls.last(), Some(&WATCH_POLL), "idle waits block");
        assert!(stdout.contains("watching (Ctrl-C to stop)"), "{stdout}");
    }

    #[test]
    fn watch_run_json_ends_each_replaced_iteration_in_command_finished() {
        let runner = AppRunner::new();
        let mut polls = 0;
        let (code, stdout, stderr) = run_app_watch(
            &["watch", "run", "//app:bin", "--output=json"],
            &runner,
            |_, _| {
                polls += 1;
                match polls {
                    2 => Ok(vec![PathBuf::from("/ws/src/main.rs")]),
                    4 => Err(AdoptError::WatchFailed {
                        detail: "stop".to_owned(),
                    }),
                    _ => Ok(Vec::new()),
                }
            },
        );
        assert_eq!(code, 1);
        let events = json_events(&stdout);
        assert_eq!(
            event_kinds(&events)
                .iter()
                .filter(|kind| **kind == "command_started")
                .count(),
            2,
            "{stdout}"
        );
        let finished = events_of_kind(&events, "command_finished");
        assert_eq!(finished.len(), 3, "{stdout}");
        assert_eq!(finished[0]["exit_code"], 143, "{stdout}");
        assert_eq!(finished[1]["exit_code"], 143, "{stdout}");
        assert_eq!(finished[2]["exit_code"], 1, "{stdout}");
        assert!(stderr.contains("changed /ws/src/main.rs"), "{stderr}");
    }

    #[test]
    fn watch_finite_commands_never_withdraw_the_running_command() {
        let runner = AppRunner::new();
        let mut waits = Vec::new();
        let (code, _, _) = run_app_watch(&["watch", "build", "//..."], &runner, |_, timeout| {
            waits.push(timeout);
            Err(AdoptError::WatchFailed {
                detail: "stop".to_owned(),
            })
        });
        assert_eq!(code, 1);
        assert_eq!(waits, [WATCH_POLL], "build is never polled mid-run");
        let events = runner.events.borrow();
        assert_eq!(events.len(), 1, "{events:?}");
        assert!(events[0].starts_with("exit "), "{events:?}");
    }

    struct SlotProbe {
        inner: CountingRunner,
        dir: PathBuf,
        busy: RefCell<Vec<(bool, bool)>>,
    }

    impl dx_process::Runner for SlotProbe {
        fn run(
            &self,
            argv: &[String],
            cwd: &Path,
            env: &[(&str, &str)],
        ) -> io::Result<dx_process::ChildStatus> {
            let busy = !crate::test_support::slot_settles_free(&self.dir, 1);
            let marked = env.contains(&(dx_plan::slots::SLOT_ENV_VAR, "1"));
            self.busy.borrow_mut().push((busy, marked));
            self.inner.run(argv, cwd, env)
        }
    }

    fn slot_watch(
        dir: &Path,
        words: &[&str],
        max_iterations: Option<u32>,
        wait_for_change: impl FnMut(
            &dx_adopt::watch::WatchSession,
            Duration,
        ) -> Result<Vec<PathBuf>, AdoptError>,
    ) -> (i32, Vec<(bool, bool)>, String) {
        let gate = dx_plan::slots::Gate::new(dir.to_path_buf(), 1, "watch-tree".to_owned(), false);
        slot_watch_through(gate, dir, words, max_iterations, wait_for_change)
    }

    fn slot_watch_through(
        gate: dx_plan::slots::Gate,
        dir: &Path,
        words: &[&str],
        max_iterations: Option<u32>,
        wait_for_change: impl FnMut(
            &dx_adopt::watch::WatchSession,
            Duration,
        ) -> Result<Vec<PathBuf>, AdoptError>,
    ) -> (i32, Vec<(bool, bool)>, String) {
        let harness = Harness::new("dx-watch-slot-");
        let runner = SlotProbe {
            inner: CountingRunner {
                seen: RefCell::new(Vec::new()),
            },
            dir: dir.to_path_buf(),
            busy: RefCell::new(Vec::new()),
        };
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = dx_plan::slots::with_gate(Some(gate), || {
            watch(
                &invocation(words),
                Env {
                    workspace: &harness.workspace,
                    runner: &runner,
                    query_runner: &NullQuery,
                    temp_dir: &harness.temp_dir,
                    pid: 1,
                    nonce: 1,
                    out: &mut out,
                    err: &mut err,
                    ci: false,
                    audit_today: None,
                    actions: &ActionTally::default(),
                    proc_root: Path::new("/proc"),
                    bump_commit_json: None,
                },
                max_iterations,
                &|| None,
                wait_for_change,
            )
        });
        let busy = runner.busy.into_inner();
        (code, busy, String::from_utf8(err).expect("stderr"))
    }

    #[test]
    fn watch_holds_a_slot_per_iteration_and_none_while_idle() {
        let scratch = dx_test_scratch::scratch("dx-watch-slot-idle-");
        let dir = scratch.path().join("slots");
        let idle_checks = std::cell::Cell::new(0);
        let (code, busy, err) = slot_watch(
            &dir,
            &["watch", "build", "//a:one"],
            Some(2),
            |_, timeout| {
                if timeout > Duration::ZERO {
                    assert!(
                        crate::test_support::slot_settles_free(&dir, 1),
                        "watch holds a slot while idle"
                    );
                    idle_checks.set(idle_checks.get() + 1);
                }
                Ok(vec![PathBuf::from("/ws/a.rs")])
            },
        );
        assert_eq!(code, 0, "{err}");
        assert_eq!(
            busy,
            [(true, true), (true, true)],
            "each iteration builds under a slot its Bazel child sees"
        );
        assert_eq!(idle_checks.get(), 1);
        assert!(crate::test_support::slot_settles_free(&dir, 1));
    }

    #[test]
    fn each_watch_iteration_reclaims() {
        let scratch = dx_test_scratch::scratch("dx-watch-slot-reclaim-");
        let dir = scratch.path().join("slots");
        let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let counter = std::sync::Arc::clone(&calls);
        let gate = dx_plan::slots::Gate::new(dir.clone(), 1, "watch-tree".to_owned(), false)
            .reclaiming(move || {
                counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                Some("low disk: reclaimed".to_owned())
            });
        let (code, busy, err) = slot_watch_through(
            gate,
            &dir,
            &["watch", "build", "//a:one"],
            Some(2),
            |_, _| Ok(vec![PathBuf::from("/ws/a.rs")]),
        );
        assert_eq!(code, 0, "{err}");
        assert_eq!(busy.len(), 2);
        assert_eq!(
            calls.load(std::sync::atomic::Ordering::SeqCst),
            2,
            "one reclaim per iteration"
        );
        assert_eq!(err.matches("dx: low disk: reclaimed\n").count(), 2, "{err}");
    }

    #[test]
    fn watch_iteration_waits_for_a_taken_slot() {
        let scratch = dx_test_scratch::scratch("dx-watch-slot-wait-");
        let dir = scratch.path().join("slots");
        let held = dx_plan::slots::acquire(&dir, 1, "tree-a", &mut |_| {}).expect("hold");
        let releaser = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(600));
            drop(held);
        });
        let (code, busy, err) =
            slot_watch(&dir, &["watch", "build", "//a:one"], Some(1), |_, _| {
                Ok(Vec::new())
            });
        releaser.join().expect("join");
        assert_eq!(code, 0, "{err}");
        assert_eq!(busy, [(true, true)]);
        assert_eq!(
            err.matches("dx: waiting for a build slot (held by tree-a)")
                .count(),
            1,
            "{err}"
        );
    }
}

#[cfg(test)]
#[cfg(unix)]
#[path = "watch_probe_tests.rs"]
mod probe_tests;
