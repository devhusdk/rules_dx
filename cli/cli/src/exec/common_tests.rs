#[test]
fn from_bazel_exit_classifies_every_code() {
    let table = [
        (0, false, PhaseKind::Clean),
        (0, true, PhaseKind::Clean),
        (1, false, PhaseKind::Incomplete),
        (2, false, PhaseKind::Selection),
        (3, false, PhaseKind::Incomplete),
        (3, true, PhaseKind::Findings),
        (4, false, PhaseKind::Selection),
        (8, false, PhaseKind::Cancelled),
        (33, false, PhaseKind::Host),
        (36, false, PhaseKind::Tool),
        (37, false, PhaseKind::Host),
    ];
    for (code, findings, kind) in table {
        assert_eq!(PhaseKind::from_bazel_exit(code, findings), kind, "{code}");
    }
}

use super::*;

#[test]
fn actions_line_formats_action_counts() {
    let tally = ActionTally::default();
    let mut err = Vec::new();
    print_actions(OutputMode::Text { quiet: false }, &tally, &mut err);
    assert!(err.is_empty());
    let counts = ActionCounts {
        created: 50,
        executed: 3,
        disk: 1,
        remote: 2,
        local: 4,
    };
    tally.record(counts);
    tally.record(counts);
    print_actions(OutputMode::Json, &tally, &mut err);
    print_actions(OutputMode::Text { quiet: true }, &tally, &mut err);
    assert!(err.is_empty());
    print_actions(OutputMode::Text { quiet: false }, &tally, &mut err);
    assert_eq!(
        String::from_utf8(err).expect("utf8"),
        "dx: 20 actions, 6 executed, 14 cached\n"
    );
}
use dx_output::{FinishedCounts, command_finished};
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

struct Flaky {
    written: Vec<u8>,
    calls: usize,
    fail_on: usize,
    kind: io::ErrorKind,
}

impl Write for Flaky {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.calls += 1;
        if self.calls == self.fail_on {
            return Err(io::Error::new(self.kind, "stdout failed"));
        }
        self.written.extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn phase_effects_touched_counts_changes_and_refusals() {
    assert!(!PhaseEffects::default().touched());
    let applied_only = PhaseEffects {
        applied: 1,
        ..PhaseEffects::default()
    };
    assert!(!applied_only.touched());
    for effects in [
        PhaseEffects {
            changes: 1,
            applied: 1,
            ..PhaseEffects::default()
        },
        PhaseEffects {
            not_applied: 1,
            ..PhaseEffects::default()
        },
    ] {
        assert!(effects.touched(), "{effects:?}");
    }
}

fn flaky(fail_on: usize, kind: io::ErrorKind) -> Flaky {
    Flaky {
        written: Vec::new(),
        calls: 0,
        fail_on,
        kind,
    }
}

#[test]
fn latched_out_refuses_every_write_after_a_broken_pipe() {
    let mut inner = flaky(2, io::ErrorKind::BrokenPipe);
    let mut latched = LatchedOut::new(&mut inner);
    latched.write_all(b"first\n").expect("first write lands");
    assert!(latched.write_all(b"second\n").is_err());
    assert!(latched.write_all(b"third\n").is_err());
    assert!(latched.flush().is_err());
    assert_eq!(latched.exit_code(0), 141);
    assert_eq!(inner.written, b"first\n");
    assert_eq!(inner.calls, 2, "nothing reaches stdout after the failure");
}

#[test]
fn latched_out_maps_other_failures_to_operational() {
    let mut inner = flaky(1, io::ErrorKind::Other);
    let mut latched = LatchedOut::new(&mut inner);
    assert!(latched.write_all(b"first\n").is_err());
    assert_eq!(latched.exit_code(0), 1);
}

#[test]
fn latched_out_retries_interrupted_writes_and_keeps_the_code() {
    let mut inner = flaky(1, io::ErrorKind::Interrupted);
    let mut latched = LatchedOut::new(&mut inner);
    latched
        .write_all(b"first\n")
        .expect("interrupted writes retry");
    assert_eq!(latched.exit_code(3), 3);
    assert_eq!(inner.written, b"first\n");
}

struct ProbeRunner {
    code: Option<i32>,
    signal: Option<i32>,
    spawn_error: bool,
    #[allow(clippy::type_complexity)]
    seen: Rc<RefCell<(Vec<String>, Vec<(String, String)>)>>,
    free: Option<u64>,
    probed: RefCell<Vec<PathBuf>>,
}

impl Runner for ProbeRunner {
    fn free_disk_bytes(&self, path: &Path) -> Option<u64> {
        self.probed.borrow_mut().push(path.to_path_buf());
        self.free
    }

    fn run(
        &self,
        argv: &[String],
        _cwd: &Path,
        env: &[(&str, &str)],
    ) -> io::Result<dx_process::ChildStatus> {
        if self.spawn_error {
            return Err(io::Error::other("fake launch failure"));
        }
        *self.seen.borrow_mut() = (
            argv.to_vec(),
            env.iter()
                .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
                .collect(),
        );
        Ok(dx_process::ChildStatus {
            code: self.code,
            signal: self.signal,
        })
    }
}

fn bazel_invocation() -> Invocation {
    dx_plan::args::parse(&["build".to_owned(), "//...".to_owned()]).expect("parse")
}

#[test]
fn run_bazel_forwards_argv_and_env_and_returns_the_exit_code() {
    let seen = Rc::new(RefCell::new((Vec::new(), Vec::new())));
    let runner = ProbeRunner {
        code: Some(7),
        signal: None,
        spawn_error: false,
        seen: Rc::clone(&seen),
        free: None,
        probed: RefCell::new(Vec::new()),
    };
    let invocation = bazel_invocation();
    let mut out = Vec::new();
    let mut err = Vec::new();
    let argv = vec!["bazel".to_owned(), "build".to_owned(), "//...".to_owned()];
    let code = run_bazel(
        &invocation,
        &mut out,
        &mut err,
        Path::new("/ws"),
        &runner,
        &argv,
        &[("DX_BUILD_MODE", "release")],
    );
    assert_eq!(code, Ok(7));
    let seen = seen.borrow();
    assert_eq!(seen.0, argv);
    assert_eq!(
        seen.1,
        vec![("DX_BUILD_MODE".to_owned(), "release".to_owned())]
    );
    assert_eq!(err, Vec::<u8>::new());
}

/// Records which runner method each call came through.
#[derive(Default)]
struct CallRunner {
    calls: RefCell<Vec<&'static str>>,
}

impl Runner for CallRunner {
    fn run(
        &self,
        _argv: &[String],
        _cwd: &Path,
        _env: &[(&str, &str)],
    ) -> io::Result<dx_process::ChildStatus> {
        unreachable!("builds go through run_withdrawable") // LCOV_EXCL_LINE - reason: the build never takes this path, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
    }

    fn run_withdrawable(
        &self,
        _argv: &[String],
        _cwd: &Path,
        _env: &[(&str, &str)],
        withdraw: &dyn Fn() -> bool,
    ) -> io::Result<Option<dx_process::ChildStatus>> {
        self.calls.borrow_mut().push("run_withdrawable");
        Ok((!withdraw()).then_some(dx_process::ChildStatus {
            code: Some(7),
            signal: None,
        }))
    }

    fn run_app(
        &self,
        _argv: &[String],
        _cwd: &Path,
        _env: &[(&str, &str)],
        withdraw: Option<&dyn Fn() -> bool>,
        _held: &[&std::fs::File],
    ) -> io::Result<Option<dx_process::ChildStatus>> {
        self.calls.borrow_mut().push("run_app");
        Ok(
            (!withdraw.is_some_and(|withdraw| withdraw())).then_some(dx_process::ChildStatus {
                code: Some(7),
                signal: None,
            }),
        )
    }
}

#[test]
fn a_withdrawable_build_keeps_its_stderr_and_holds_nothing() {
    let runner = CallRunner::default();
    let invocation = bazel_invocation();
    let mut out = Vec::new();
    let mut err = Vec::new();
    let argv = vec!["bazel".to_owned(), "build".to_owned(), "//...".to_owned()];
    let mut build = |withdraw: &dyn Fn() -> bool| {
        run_bazel_withdrawable(
            &invocation,
            &mut out,
            &mut err,
            Path::new("/ws"),
            &runner,
            &argv,
            withdraw,
        )
    };
    assert_eq!(build(&|| false), Ok(Some(7)));
    assert_eq!(build(&|| true), Ok(None));
    assert_eq!(*runner.calls.borrow(), ["run_withdrawable"; 2]);
}

#[test]
fn a_withdrawable_app_keeps_its_tty_and_goes_through_run_app() {
    let runner = CallRunner::default();
    let invocation = bazel_invocation();
    let mut out = Vec::new();
    let mut err = Vec::new();
    let argv = vec!["bazel".to_owned(), "run".to_owned(), "//:app".to_owned()];
    let mut app = |withdraw: &dyn Fn() -> bool| {
        run_bazel_app_withdrawable(
            &invocation,
            &mut out,
            &mut err,
            Path::new("/ws"),
            &runner,
            &argv,
            withdraw,
        )
        .map(|exit| exit.map(|exit| exit.code))
    };
    assert_eq!(app(&|| false), Ok(Some(7)));
    assert_eq!(app(&|| true), Ok(None));
    assert_eq!(*runner.calls.borrow(), ["run_app"; 2]);
}

#[test]
fn run_bazel_reports_a_spawn_failure_and_asks_the_caller_to_stop() {
    let runner = ProbeRunner {
        code: Some(0),
        signal: None,
        spawn_error: true,
        seen: Rc::new(RefCell::new((Vec::new(), Vec::new()))),
        free: None,
        probed: RefCell::new(Vec::new()),
    };
    let invocation = bazel_invocation();
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = run_bazel(
        &invocation,
        &mut out,
        &mut err,
        Path::new("/ws"),
        &runner,
        &["bazel".to_owned()],
        &[],
    );
    assert_eq!(code, Err(operational_code()));
    assert_eq!(
        String::from_utf8(err).expect("utf8"),
        "dx: launch_failed: failed to launch Bazel: fake launch failure\n\
         next: install bazelisk (or bazel) on PATH, using the version pinned in .bazelversion, and run the command again.\n"
    );
}

#[test]
fn run_bazel_reports_a_signalled_bazel_and_asks_the_caller_to_stop() {
    let runner = ProbeRunner {
        code: None,
        signal: None,
        spawn_error: false,
        seen: Rc::new(RefCell::new((Vec::new(), Vec::new()))),
        free: None,
        probed: RefCell::new(Vec::new()),
    };
    let invocation = bazel_invocation();
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = run_bazel(
        &invocation,
        &mut out,
        &mut err,
        Path::new("/ws"),
        &runner,
        &["bazel".to_owned()],
        &[],
    );
    assert_eq!(code, Err(operational_code()));
    assert_eq!(
        String::from_utf8(err).expect("utf8"),
        "dx: bazel_signalled: Bazel terminated by signal\n"
    );
}

fn disk_runner(code: i32, free: Option<u64>) -> ProbeRunner {
    ProbeRunner {
        code: Some(code),
        signal: None,
        spawn_error: false,
        seen: Rc::new(RefCell::new((Vec::new(), Vec::new()))),
        free,
        probed: RefCell::new(Vec::new()),
    }
}

fn run_on_disk(runner: &ProbeRunner, output: OutputMode) -> (Result<i32, i32>, String, String) {
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let argv = vec![
        "bazel".to_owned(),
        "--output_base=/ob".to_owned(),
        "build".to_owned(),
    ];
    let code = run_bazel_output(
        output,
        &mut out,
        &mut err,
        Path::new("/ws"),
        runner,
        &argv,
        &[],
    );
    (
        code,
        String::from_utf8(out).expect("utf8"),
        String::from_utf8(err).expect("utf8"),
    )
}

#[test]
fn run_bazel_app_keeps_exit_36_on_a_nearly_full_disk() {
    for output in [OutputMode::Text { quiet: false }, OutputMode::Json] {
        let runner = disk_runner(36, Some(1 << 20));
        let mut invocation = bazel_invocation();
        invocation.output = output;
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = run_bazel_app(
            &invocation,
            &mut out,
            &mut err,
            Path::new("/ws"),
            &runner,
            &["bazel".to_owned(), "run".to_owned(), "//app".to_owned()],
            &[],
        );
        assert_eq!(
            code,
            Ok(AppExit {
                code: 36,
                signal: None
            })
        );
        assert!(out.is_empty(), "{}", String::from_utf8_lossy(&out));
        let err = String::from_utf8(err).expect("utf8");
        assert!(!err.contains("disk_"), "{err}");
        assert!(runner.probed.borrow().is_empty());
    }
}

#[test]
fn run_bazel_app_still_reports_spawn_failures_and_signals() {
    for (spawn_error, code, expected) in [
        (true, Some(0), "launch_failed"),
        (false, None, "bazel_signalled"),
    ] {
        let runner = ProbeRunner {
            code,
            signal: None,
            spawn_error,
            seen: Rc::new(RefCell::new((Vec::new(), Vec::new()))),
            free: Some(1 << 20),
            probed: RefCell::new(Vec::new()),
        };
        let invocation = bazel_invocation();
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let exit = run_bazel_app(
            &invocation,
            &mut out,
            &mut err,
            Path::new("/ws"),
            &runner,
            &["bazel".to_owned()],
            &[],
        );
        assert_eq!(exit, Err(operational_code()));
        let err = String::from_utf8(err).expect("utf8");
        assert!(err.starts_with(&format!("dx: {expected}: ")), "{err}");
        assert!(runner.probed.borrow().is_empty());
    }
}

fn signal_runner(signal: Option<i32>) -> ProbeRunner {
    ProbeRunner {
        code: None,
        signal,
        spawn_error: false,
        seen: Rc::new(RefCell::new((Vec::new(), Vec::new()))),
        free: None,
        probed: RefCell::new(Vec::new()),
    }
}

#[test]
fn run_bazel_app_maps_a_signalled_app_to_128_plus_the_signal() {
    for output in [OutputMode::Text { quiet: false }, OutputMode::Json] {
        let runner = signal_runner(Some(11));
        let mut invocation = bazel_invocation();
        invocation.output = output;
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let exit = run_bazel_app(
            &invocation,
            &mut out,
            &mut err,
            Path::new("/ws"),
            &runner,
            &["bazel".to_owned(), "run".to_owned(), "//app".to_owned()],
            &[],
        );
        assert_eq!(
            exit,
            Ok(AppExit {
                code: 139,
                signal: Some(11)
            })
        );
        assert!(out.is_empty(), "{}", String::from_utf8_lossy(&out));
        assert!(err.is_empty(), "{}", String::from_utf8_lossy(&err));
    }
}

#[test]
fn run_bazel_keeps_bazel_signalled_when_the_signal_is_known() {
    let runner = signal_runner(Some(11));
    let invocation = bazel_invocation();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = run_bazel(
        &invocation,
        &mut out,
        &mut err,
        Path::new("/ws"),
        &runner,
        &["bazel".to_owned()],
        &[],
    );
    assert_eq!(code, Err(operational_code()));
    assert_eq!(
        String::from_utf8(err).expect("utf8"),
        "dx: bazel_signalled: Bazel terminated by signal\n"
    );
}

#[test]
fn disk_full_names_exit_36_on_a_nearly_full_disk() {
    let runner = disk_runner(36, Some(1 << 20));
    let (code, _, err) = run_on_disk(&runner, OutputMode::Text { quiet: false });
    assert_eq!(code, Err(36));
    assert!(
        err.contains(
            "dx: disk_low: 1.0 MiB free at /ws; free disk space with `dx worktree prune --apply`\n"
        ),
        "{err}"
    );
    assert!(
        err.contains("dx: disk_full: Bazel exited 36 with 1.0 MiB free at /ws; the disk is full\n"),
        "{err}"
    );
    assert!(
        err.contains(
            "next: free disk space (try `dx worktree prune --apply`) and run the command again.\n"
        ),
        "{err}"
    );
    assert!(runner.probed.borrow().contains(&PathBuf::from("/ob")));

    let runner = disk_runner(36, Some(1 << 20));
    let (code, out, _) = run_on_disk(&runner, OutputMode::Json);
    assert_eq!(code, Err(36));
    let events: Vec<serde_json::Value> = out
        .lines()
        .map(|line| serde_json::from_str(line).expect("NDJSON"))
        .collect();
    let notices: Vec<_> = events.iter().filter(|e| e["event"] == "notice").collect();
    assert_eq!(notices.len(), 1, "{out}");
    assert_eq!(notices[0]["code"], "disk_low");
    assert_eq!(notices[0]["severity"], "warning");
    let errors: Vec<_> = events.iter().filter(|e| e["event"] == "error").collect();
    assert_eq!(errors.len(), 1, "{out}");
    assert_eq!(errors[0]["code"], "disk_full");
    assert_eq!(errors[0]["cause"], "host");
    assert_eq!(errors[0]["status"], 36);
    assert_eq!(errors[0]["phase"], "execute");
    let finished: Vec<_> = events
        .iter()
        .filter(|e| e["event"] == "command_finished")
        .collect();
    assert_eq!(finished.len(), 1, "{out}");
    assert_eq!(finished[0]["exit_code"], 36);
}

#[test]
fn exit_36_with_room_on_disk_keeps_the_bazel_exit() {
    let runner = disk_runner(36, Some(100 << 30));
    let (code, out, err) = run_on_disk(&runner, OutputMode::Json);
    assert_eq!(code, Ok(36));
    assert_eq!(out, "");
    assert_eq!(err, "");
}

#[test]
fn other_exits_on_a_nearly_full_disk_only_warn() {
    let runner = disk_runner(1, Some(1 << 20));
    let (code, _, err) = run_on_disk(&runner, OutputMode::Text { quiet: false });
    assert_eq!(code, Ok(1));
    assert!(err.starts_with("dx: disk_low: "), "{err}");
    assert!(!err.contains("disk_full"), "{err}");
    assert_eq!(
        runner.probed.borrow().len(),
        dx_plan::disk::disk_paths(Path::new("/ws"), &["--output_base=/ob".to_owned()]).len()
    );
}

#[test]
fn exit_36_without_a_disk_reading_keeps_the_bazel_exit() {
    let runner = disk_runner(36, None);
    let (code, out, err) = run_on_disk(&runner, OutputMode::Json);
    assert_eq!(code, Ok(36));
    assert_eq!(out, "");
    assert_eq!(err, "");
}

#[test]
fn hex_digest_formats_lowercase_hex() {
    assert_eq!(hex_digest(&[0xabu8; 32]), "ab".repeat(32));
    assert_eq!(hex_digest(&[0u8; 32]).len(), 64);
}

#[test]
fn apply_rejects_overlapping_edits_without_writing() {
    let original = b"abcdef";
    assert!(apply_to_bytes(original, &[(0, 2, "AB".to_owned())]).is_some());
    assert!(apply_to_bytes(original, &[(0, 2, "AB".to_owned()), (1, 3, "X".to_owned())]).is_none());
    assert!(apply_to_bytes(original, &[(0, 6, "AB".to_owned()), (6, 7, "X".to_owned())]).is_none());
    assert!(apply_to_bytes(b"\xff\xfe", &[(0, 1, "a".to_owned())]).is_none());
}

fn finding(path: Option<&str>, range: Option<(u64, u64)>) -> FindingRecord {
    FindingRecord::single(DiagnosticEvent {
        rule: Some("rule".to_owned()),
        path: path.map(str::to_owned),
        range,
        fixable: true,
        ..DiagnosticEvent::new(Severity::Warning, "lint-tool", "mapped")
    })
}

fn at(line: u64, column: u64) -> dx_output::Position {
    dx_output::Position { line, column }
}

#[test]
fn text_finding_prints_the_verified_location_tool_rule_severity_and_message() {
    let mut record = finding(Some("src/a.py"), Some((4, 6)));
    record.span = Some((at(2, 3), at(2, 5)));
    assert_eq!(
        text_finding(&record),
        "src/a.py:2:3: warning: mapped [lint-tool/rule] (fixable)"
    );
    record.count = 3;
    record.diagnostic.fixable = false;
    assert_eq!(
        text_finding(&record),
        "src/a.py:2:3: warning: mapped [lint-tool/rule] (x3)"
    );
}

#[test]
fn text_finding_colors_severity_only_when_asked() {
    let mut record = finding(Some("src/a.py"), None);
    record.diagnostic.severity = Severity::Error;
    assert!(text_finding_colored(&record, true).contains("\x1b[31merror\x1b[0m"));
    assert!(!text_finding_colored(&record, false).contains('\x1b'));
}

#[test]
fn summary_counts_findings_files_and_fixable() {
    let mut error = DiagnosticEvent::new(Severity::Error, "t", "m");
    error.path = Some("a.py".to_owned());
    let mut warning = DiagnosticEvent::new(Severity::Warning, "t", "m");
    warning.path = Some("a.py".to_owned());
    warning.fixable = true;
    assert_eq!(
        findings_summary(&[error, warning], 3),
        "1 error(s), 1 warning(s) in 1 file(s), 1 fixable with --apply"
    );
    assert_eq!(findings_summary(&[], 3), "no findings in 3 target(s)");
}

#[test]
fn text_finding_does_not_repeat_a_tool_prefix_in_the_rule() {
    let mut record = finding(Some("src/a.py"), None);
    record.diagnostic.tool = "vuln".to_owned();
    record.diagnostic.rule = Some("vuln/GHSA-1".to_owned());
    assert!(text_finding(&record).contains("[vuln/GHSA-1]"));
}

#[test]
fn text_finding_says_why_a_finding_is_not_fixable() {
    let cases = [
        (
            "unstable",
            "src/a.py: warning: mapped [lint-tool/rule] (not fixable: unstable)",
        ),
        (
            "no_change",
            "src/a.py: warning: mapped [lint-tool/rule] (not fixable: no_change)",
        ),
        (
            "persists",
            "src/a.py: warning: mapped [lint-tool/rule] (not fixable: persists)",
        ),
    ];
    for (reason, line) in cases {
        let mut record = finding(Some("src/a.py"), None);
        record.diagnostic.fixable = false;
        record.diagnostic.unfixable_reason = Some(reason.to_owned());
        assert_eq!(text_finding(&record), line);
    }
}

#[test]
fn text_finding_never_guesses_a_location() {
    let unverified = finding(Some("src/a.py"), Some((4, 6)));
    assert_eq!(
        text_finding(&unverified),
        "src/a.py: warning: mapped [lint-tool/rule] (fixable)"
    );
    let mut pathless = finding(None, None);
    pathless.diagnostic.rule = None;
    pathless.diagnostic.fixable = false;
    assert_eq!(
        text_finding(&pathless),
        "(no file): warning: mapped [lint-tool]"
    );
    let mut stray = finding(Some("src/a.py"), None);
    stray.span = Some((at(9, 9), at(9, 9)));
    assert!(text_finding(&stray).starts_with("src/a.py: warning"));
}

#[test]
fn text_finding_places_related_items_and_suggestions() {
    let mut record = finding(None, None);
    record.diagnostic.fixable = false;
    record.diagnostic.related = vec![
        dx_output::RelatedEvent {
            role: "label".to_owned(),
            message: "expected `i32`".to_owned(),
            path: Some("src/b.rs".to_owned()),
            range: Some((1, 2)),
            replacement: None,
            applicability: None,
        },
        dx_output::RelatedEvent {
            role: "help".to_owned(),
            message: "try this".to_owned(),
            path: None,
            range: None,
            replacement: Some("1".to_owned()),
            applicability: Some("MaybeIncorrect".to_owned()),
        },
    ];
    record.related_spans = vec![Some((at(7, 2), at(7, 3))), None];
    assert_eq!(
        text_finding(&record),
        "(no file): warning: mapped [lint-tool/rule]\n  label: expected `i32` (src/b.rs:7:2)\n  help: try this suggests `1` [MaybeIncorrect]"
    );
}

#[test]
fn apply_rejects_boundary_and_encoding_violations() {
    assert!(apply_to_bytes("héllo".as_bytes(), &[(2, 3, "X".to_owned())]).is_none());
    assert!(apply_to_bytes("héllo".as_bytes(), &[(1, 2, "X".to_owned())]).is_none());
    assert!(apply_to_bytes(b"\xff", &[(0, 1, "X".to_owned())]).is_none());
    assert_eq!(
        apply_to_bytes(b"ab", &[(0, 1, "X".to_owned())]),
        Some(b"Xb".to_vec())
    );
}

#[test]
fn change_event_is_deterministic_and_reconstructs_candidate() {
    let original = b"BAD\n";
    let terminal = "GOOD\n";
    let change = FileChange {
        path: "src/lib.rs".to_owned(),
        original_digest: digest(original),
        edits: vec![(0, original.len() as u64, terminal.to_owned())],
    };
    let first = change_event_for(&change);
    let second = change_event_for(&change);
    assert_eq!(first.path, "src/lib.rs");
    assert_eq!(first.path, second.path);
    assert_eq!(first.source_digest, second.source_digest);
    assert_eq!(first.source_digest, Some(hex_digest(&digest(original))));
    assert_eq!(first.edits.len(), 1);
    assert_eq!(first.edits[0].start, 0);
    assert_eq!(first.edits[0].end, original.len() as u64);
    assert_eq!(first.edits[0].replacement, terminal);
    assert_eq!(
        first, second,
        "the event must not depend on how many times it is built"
    );
    let planned = apply_to_bytes(original, &change.edits).expect("apply");
    assert_eq!(planned, terminal.as_bytes());
    let other = FileChange {
        path: "src/other.rs".to_owned(),
        original_digest: digest(original),
        edits: vec![(0, original.len() as u64, terminal.to_owned())],
    };
    let other_event = change_event_for(&other);
    assert_ne!(first.path, other_event.path);
}

struct BrokenPipeWriter;

impl Write for BrokenPipeWriter {
    fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
        Err(io::Error::new(io::ErrorKind::BrokenPipe, "broken pipe"))
    }

    fn flush(&mut self) -> io::Result<()> {
        Err(io::Error::new(io::ErrorKind::BrokenPipe, "broken pipe"))
    }
}

struct FailingWriter;

impl Write for FailingWriter {
    fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
        Err(io::Error::other("boom"))
    }

    fn flush(&mut self) -> io::Result<()> {
        Err(io::Error::other("boom"))
    }
}

#[test]
fn stdout_helpers_map_broken_pipe_to_141() {
    assert_eq!(dx_process::broken_pipe_code(), 128 + 13);
    let broken = io::Error::new(io::ErrorKind::BrokenPipe, "broken pipe");
    assert_eq!(stdout_io_code(&broken), 128 + 13);
    let other = io::Error::other("boom");
    assert_eq!(stdout_io_code(&other), operational_code());
    let broken_out = dx_output::OutputError::from(io::Error::from(io::ErrorKind::BrokenPipe));
    assert!(broken_out.is_broken_pipe());
    assert_eq!(stdout_output_code(&broken_out), 128 + 13);
    let other_out = dx_output::OutputError::from(io::Error::other("boom"));
    assert!(!other_out.is_broken_pipe());
    assert_eq!(stdout_output_code(&other_out), operational_code());
}

#[test]
fn emit_event_and_flush_fail_with_broken_pipe_code() {
    let event = command_finished(0, &FinishedCounts::default());
    assert_eq!(emit_event(&mut BrokenPipeWriter, &event), Err(128 + 13));
    assert_eq!(emit_event(&mut FailingWriter, &event), Err(1));
    assert_eq!(flush_out(&mut BrokenPipeWriter), Err(128 + 13));
    assert_eq!(flush_out(&mut FailingWriter), Err(1));
    assert!(
        check_stdout_write(Err(io::Error::new(
            io::ErrorKind::BrokenPipe,
            "broken pipe"
        )))
        .is_err()
    );
}

#[test]
fn report_failed_writes_the_line_and_only_json_emits_the_event() {
    let mut out = Vec::new();
    let mut err = Vec::new();
    assert_eq!(
        report_failed(
            &mut out,
            &mut err,
            OutputMode::Text { quiet: false },
            "boom",
        ),
        Ok(())
    );
    assert_eq!(
        String::from_utf8(err).unwrap(),
        "dx: report_failed: boom\nnext: make the report destination writable and run the command again.\n"
    );
    assert!(out.is_empty());

    let mut out = Vec::new();
    let mut err = Vec::new();
    assert_eq!(
        report_failed(&mut out, &mut err, OutputMode::Diff, "boom"),
        Ok(())
    );
    assert_eq!(
        String::from_utf8(err).unwrap(),
        "dx: report_failed: boom\nnext: make the report destination writable and run the command again.\n"
    );
    assert!(out.is_empty());

    let mut out = Vec::new();
    let mut err = Vec::new();
    assert_eq!(
        report_failed(&mut out, &mut err, OutputMode::Json, "boom"),
        Ok(())
    );
    assert_eq!(
        String::from_utf8(err).unwrap(),
        "dx: report_failed: boom\nnext: make the report destination writable and run the command again.\n"
    );
    let event: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(event["code"], CODE_REPORT_FAILED);
    assert_eq!(event["message"], "boom");
}

#[test]
fn report_failed_with_next_replaces_the_default_next_step() {
    let mut err = Vec::new();
    assert_eq!(
        report_failed_with_next(
            &mut Vec::new(),
            &mut err,
            OutputMode::Text { quiet: false },
            "boom",
            Some(super::super::guidance::EVIDENCE_GAP_NEXT),
        ),
        Ok(())
    );
    let text = String::from_utf8(err).unwrap();
    assert!(text.contains("fix the test or coverage problem"), "{text}");
    assert!(!text.contains("writable"), "{text}");
}

#[test]
fn report_failed_returns_the_stdout_code_when_its_event_cannot_land() {
    let mut err = Vec::new();
    assert_eq!(
        report_failed(&mut BrokenPipeWriter, &mut err, OutputMode::Json, "boom"),
        Err(128 + 13)
    );
}

#[test]
fn emit_built_writes_a_built_event_like_emit_event() {
    let event = command_finished(0, &FinishedCounts::default());
    let mut out = Vec::new();
    assert_eq!(emit_built(&mut out, Ok(event.clone())), Ok(()));
    let written: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(written, event);
    assert_eq!(
        emit_built(&mut BrokenPipeWriter, Ok(event.clone())),
        emit_event(&mut BrokenPipeWriter, &event)
    );
    assert_eq!(emit_built(&mut BrokenPipeWriter, Ok(event)), Err(128 + 13));
}

#[test]
fn emit_built_returns_the_write_code_when_the_invalid_result_cannot_land() {
    assert_eq!(
        emit_built(&mut BrokenPipeWriter, command_started("", "default")),
        Err(128 + 13)
    );
    assert_eq!(
        emit_built(&mut FailingWriter, command_started("", "default")),
        Err(operational_code())
    );
}

#[test]
fn emit_built_ends_the_stream_when_an_event_fails_to_build() {
    let mut out = Vec::new();
    let built = command_started("", "default");
    assert!(built.is_err());
    assert_eq!(emit_built(&mut out, built), Err(operational_code()));
    let lines: Vec<serde_json::Value> = String::from_utf8(out)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(lines.len(), 2, "{lines:?}");
    assert_eq!(lines[0]["event"], "error");
    assert_eq!(lines[0]["code"], CODE_INVALID_RESULT);
    assert_eq!(lines[1]["event"], "command_finished");
    assert_eq!(lines[1]["results_complete"], false);
    assert_eq!(lines[1]["exit_code"], operational_code());
}

#[test]
fn a_later_flush_failure_keeps_the_earlier_write_failure() {
    struct WriteThenFlush;
    impl Write for WriteThenFlush {
        fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
            Err(io::Error::from(io::ErrorKind::PermissionDenied))
        }
        fn flush(&mut self) -> io::Result<()> {
            Err(io::Error::from(io::ErrorKind::BrokenPipe))
        }
    }
    let mut inner = WriteThenFlush;
    let mut latched = LatchedOut::new(&mut inner);
    assert!(latched.write_all(b"x").is_err());
    assert_eq!(latched.exit_code(0), operational_code());
}

#[test]
fn a_flush_failure_alone_fails_a_clean_run() {
    let mut inner = BrokenPipeWriter;
    let mut latched = LatchedOut::new(&mut inner);
    assert_eq!(latched.exit_code(0), 128 + 13);
}

#[test]
fn write_report_document_honours_the_destination() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut out = Vec::new();
    write_report_document(
        &mut out,
        dir.path(),
        &Destination::File("out.sarif".to_owned()),
        "{}\n",
    )
    .expect("write");
    assert_eq!(
        std::fs::read_to_string(dir.path().join("out.sarif")).expect("report"),
        "{}\n"
    );
    assert!(out.is_empty());

    write_report_document(&mut out, dir.path(), &Destination::Stdout, "{}").expect("stdout");
    assert_eq!(String::from_utf8(out).expect("utf8"), "{}\n");

    let err = write_report_document(
        &mut BrokenPipeWriter,
        dir.path(),
        &Destination::Stdout,
        "{}\n",
    )
    .expect_err("broken pipe");
    assert_eq!(err.destination, "-");
    assert!(err.reason.contains("broken pipe"), "{err}");
}

#[test]
fn write_report_file_refuses_a_parent_that_is_not_a_directory() {
    let dir = tempfile::tempdir().expect("tempdir");
    let err = write_report_file(dir.path(), "nested/out.sarif", "{}\n").expect_err("no parent");
    assert!(err.destination.contains("nested"), "{err}");
    assert!(err.destination.contains("out.sarif"), "{err}");
    assert!(err.reason.contains("parent directory"), "{err}");
    assert!(err.reason.contains("does not exist"), "{err}");
    assert!(!dir.path().join("nested").exists());
    std::fs::create_dir(dir.path().join("nested")).expect("mkdir");
    write_report_file(dir.path(), "nested/out.sarif", "{}\n").expect("write");
    assert_eq!(
        std::fs::read_to_string(dir.path().join("nested/out.sarif")).expect("report"),
        "{}\n"
    );
    let err = write_report_file(dir.path(), "nested", "{}\n").expect_err("is a directory");
    assert!(err.reason.contains("destination is a directory"), "{err}");
}

#[test]
fn write_report_file_keeps_stale_content_on_failure() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join("out.sarif"), "stale\n").expect("stale");
    std::fs::create_dir(dir.path().join("blocked")).expect("mkdir");
    let err = write_report_file(dir.path(), "blocked", "{}\n").expect_err("directory");
    assert!(err.reason.contains("destination is a directory"), "{err}");
    assert_eq!(
        std::fs::read_to_string(dir.path().join("out.sarif")).expect("stale kept"),
        "stale\n"
    );
}

#[test]
#[cfg(unix)]
fn write_report_file_reports_unwritable_destinations() {
    let dir = tempfile::tempdir().expect("tempdir");
    let locked = dir.path().join("locked");
    std::fs::create_dir(&locked).expect("mkdir");
    let mut permissions = std::fs::metadata(&locked).expect("metadata").permissions();
    permissions.set_readonly(true);
    std::fs::set_permissions(&locked, permissions).expect("readonly");
    let result = write_report_file(dir.path(), "locked/out.sarif", "{}\n");
    let mut permissions = std::fs::metadata(&locked).expect("metadata").permissions();
    #[allow(clippy::permissions_set_readonly_false)]
    permissions.set_readonly(false);
    std::fs::set_permissions(&locked, permissions).expect("writable");
    let err = result.expect_err("unwritable");
    assert!(err.destination.contains("locked"), "{err}");
    assert!(!err.reason.is_empty(), "{err}");
    assert!(!locked.join("out.sarif").exists());
}

#[test]
fn write_report_file_resolves_absolute_destinations_as_given() {
    let dir = tempfile::tempdir().expect("tempdir");
    let absolute = dir.path().join("abs.sarif").display().to_string();
    write_report_file(dir.path(), &absolute, "{}\n").expect("absolute write");
    assert_eq!(
        std::fs::read_to_string(dir.path().join("abs.sarif")).expect("report"),
        "{}\n"
    );
}

#[test]
fn collect_targets_names_the_failure_by_stage() {
    let dir = tempfile::tempdir().expect("tempdir");
    let bep = dir.path().join("missing.json");
    let error = collect_targets(&bep, "dx_results", dir.path()).expect_err("absent");
    let message = error.message();
    assert_eq!(error.exit_code(), 1);
    assert_eq!(bep_error_code(&error), CODE_UNREADABLE_BEP);
    assert!(message.contains("failed to read build events"), "{message}");

    std::fs::write(&bep, "").expect("bep");
    let error = collect_targets(&bep, "", dir.path()).expect_err("empty group");
    let message = error.message();
    assert_eq!(error.exit_code(), 1);
    assert!(message.contains("invalid BEP config"), "{message}");

    std::fs::write(&bep, "{not json").expect("bep");
    let error = collect_targets(&bep, "dx_results", dir.path()).expect_err("bad");
    let message = error.message();
    assert_eq!(error.exit_code(), 1);
    assert!(message.contains("invalid build events"), "{message}");
}

#[test]
fn collect_targets_rejects_a_cut_off_last_line() {
    let dir = tempfile::tempdir().expect("tempdir");
    let bep = dir.path().join("cut.json");
    std::fs::write(
        &bep,
        "{\"id\": {\"namedSetOfFiles\": {\"id\": \"1\"}}}\n{\"id\":{\"targetCo",
    )
    .expect("bep");
    let error = collect_targets(&bep, "dx_results", dir.path()).expect_err("cut");
    let message = error.message();
    assert_eq!(error.exit_code(), 1);
    assert!(
        message.starts_with("invalid build events: line 2: invalid JSON"),
        "{message}"
    );
}

#[test]
fn offline_summary_marks_only_offline_invocations() {
    assert_eq!(
        offline_summary("Running audit security for //...".to_owned(), true),
        "Running audit security for //... (offline, cache-only)"
    );
    assert_eq!(
        offline_summary("Running audit security for //...".to_owned(), false),
        "Running audit security for //..."
    );
}

#[test]
fn frozen_summary_marks_only_frozen_invocations() {
    assert_eq!(
        frozen_summary("Running update for //...".to_owned(), true),
        "Running update for //... (frozen, no resolution changes)"
    );
    assert_eq!(
        frozen_summary("Running update for //...".to_owned(), false),
        "Running update for //..."
    );
    assert_eq!(
        frozen_summary(
            offline_summary("Running update for //...".to_owned(), true),
            true
        ),
        "Running update for //... (offline, cache-only) (frozen, no resolution changes)"
    );
}

#[test]
fn startup_outcome_emits_error_then_finished_with_incomplete_results() {
    let mut out = Vec::new();
    emit_startup_outcome(&mut out, "invalid_arguments", "boom", 2).expect("events");
    let text = String::from_utf8(out).expect("utf8");
    let mut lines = text.lines();
    let first: serde_json::Value =
        serde_json::from_str(lines.next().expect("error")).expect("json");
    assert_eq!(first["event"], "error");
    assert_eq!(first["code"], "invalid_arguments");
    assert_eq!(first["message"], "boom");
    let second: serde_json::Value =
        serde_json::from_str(lines.next().expect("finished")).expect("json");
    assert_eq!(second["event"], "command_finished");
    assert_eq!(second["exit_code"], 2);
    assert_eq!(second["results_complete"], false);
    assert!(lines.next().is_none(), "nothing follows the terminal event");
}

#[test]
fn startup_outcome_still_finishes_when_the_code_is_empty() {
    let mut out = Vec::new();
    emit_startup_outcome(&mut out, "", "boom", 2).expect("finished anyway");
    let lines: Vec<serde_json::Value> = String::from_utf8(out)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).expect("json"))
        .collect();
    assert_eq!(lines.len(), 2, "{lines:?}");
    assert_eq!(lines[0]["event"], "error");
    assert_eq!(lines[0]["code"], CODE_INVALID_RESULT);
    assert_eq!(lines[1]["event"], "command_finished");
    assert_eq!(lines[1]["exit_code"], 2);
}

#[test]
fn startup_outcome_reports_a_broken_stdout() {
    assert_eq!(
        emit_startup_outcome(&mut BrokenPipeWriter, "invalid_arguments", "boom", 2),
        Err(128 + 13)
    );
    assert_eq!(
        emit_startup_outcome(&mut FailingWriter, "invalid_arguments", "boom", 2),
        Err(1)
    );
}

#[test]
fn operational_returns_broken_pipe_on_truncated_stdout() {
    use dx_plan::args::parse;
    let invocation = parse(&["status".to_owned(), "--output=json".to_owned()]).expect("parse");
    let mut err = Vec::new();
    let code = operational(
        &invocation,
        &mut BrokenPipeWriter,
        &mut err,
        "launch_failed",
        "boom",
    );
    assert_eq!(code, 128 + 13);
}

#[test]
fn report_dx_error_exits_by_kind_and_keeps_the_code() {
    let table = [
        (dx_error::DxError::usage("bad flag"), 2),
        (dx_error::DxError::failure("refused"), 1),
        (dx_error::DxError::infra("disk gone"), 3),
    ];
    for (error, exit) in table {
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let got = report_dx_error(OutputMode::Json, &mut out, &mut err, "update.test", &error);
        assert_eq!(got, exit, "{error}");
        let text = String::from_utf8(out).unwrap();
        assert!(text.contains("update.test"), "{text}");
        assert!(text.contains(error.message()), "{text}");
    }
}
