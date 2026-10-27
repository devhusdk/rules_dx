//! Restarts a Bazel call that crashed before its command started.
//!
//! Bazel can crash its server while it takes the shared lock on the repo
//! contents cache ("could not acquire lock on repo contents cache", then
//! "Server terminated abruptly"). The client exits 37. The command never ran,
//! so running it again is safe. A crash after the command started is never
//! repeated.

use std::cell::RefCell;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use dx_plan::resolve::{QueryResult, QueryRunner};
use dx_process::{ChildStatus, Runner};

/// The client exit code for a server that died abruptly.
pub const RESTART_EXIT: i32 = 37;

/// The most restarts after the first attempt.
pub const MAX_RESTARTS: u32 = 2;

const SIGNATURES: &[&str] = &[
    "could not acquire lock on repo contents cache",
    "Server terminated abruptly",
];

/// The wait before restart number `n` (1-based), without jitter.
pub fn backoff(n: u32) -> Duration {
    Duration::from_secs(if n <= 1 { 2 } else { 5 })
}

/// The path of the build event file named in `argv`.
pub fn bep_path_of(argv: &[String]) -> Option<&str> {
    let prefix = format!("--{}=", dx_plan::plan::BEP_FLAG_NAME);
    argv.iter()
        .rev()
        .find_map(|arg| arg.strip_prefix(prefix.as_str()))
}

/// What shows whether the crashed command had started.
pub enum Evidence<'a> {
    NotStarted,
    Started,
    Unknown,
    Stderr(&'a [u8]),
}

/// Reports whether a call that ended with `code` may run again.
pub fn restartable(code: Option<i32>, evidence: Evidence<'_>) -> bool {
    if code != Some(RESTART_EXIT) {
        return false;
    }
    match evidence {
        Evidence::NotStarted => true,
        Evidence::Started | Evidence::Unknown => false,
        Evidence::Stderr(bytes) => {
            let text = String::from_utf8_lossy(bytes);
            SIGNATURES.iter().any(|sig| text.contains(sig))
        }
    }
}

fn is_bazel(argv: &[String]) -> bool {
    argv.first()
        .is_some_and(|arg0| arg0 == dx_process::launcher_argv0())
}

fn jittered(n: u32) -> Duration {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.subsec_nanos());
    backoff(n) + Duration::from_millis(u64::from(nanos % 500))
}

fn real_sleep(total: Duration) {
    let slice = Duration::from_millis(100);
    let mut left = total;
    while !left.is_zero() && dx_process::interrupted().is_none() {
        let step = left.min(slice);
        std::thread::sleep(step);
        left -= step;
    }
}

struct Policy {
    sleep: Box<dyn Fn(Duration)>,
    stop: Box<dyn Fn() -> bool>,
    notice: RefCell<Box<dyn Write>>,
}

impl Policy {
    fn real() -> Self {
        Policy {
            sleep: Box::new(real_sleep),
            stop: Box::new(|| dx_process::interrupted().is_some()),
            notice: RefCell::new(Box::new(io::stderr())),
        }
    }

    /// Announces restart `n` and waits; false when a stop arrived first.
    fn pause(&self, code: i32, n: u32) -> bool {
        let wait = jittered(n);
        let _ = writeln!(
            self.notice.borrow_mut(),
            "dx: notice: bazel exited {code} before the command started; restarting in {}s (restart {n} of {MAX_RESTARTS})",
            wait.as_secs()
        );
        (self.sleep)(wait);
        !(self.stop)()
    }
}

/// A `Runner` that restarts Bazel calls that crashed before they started.
pub struct RestartingRunner<R> {
    inner: R,
    policy: Policy,
}

impl<R: Runner> RestartingRunner<R> {
    pub fn new(inner: R) -> Self {
        RestartingRunner {
            inner,
            policy: Policy::real(),
        }
    }

    #[cfg(test)]
    fn with_policy(inner: R, policy: Policy) -> Self {
        RestartingRunner { inner, policy }
    }

    fn evidence(argv: &[String], cwd: &Path) -> Evidence<'static> {
        let Some(path) = bep_path_of(argv) else {
            return Evidence::Unknown;
        };
        let path: PathBuf = cwd.join(path);
        match dx_bep::stream_started(&path) {
            Ok(true) => Evidence::Started,
            Ok(false) => Evidence::NotStarted,
            Err(error) if error.kind() == io::ErrorKind::NotFound => Evidence::NotStarted,
            Err(_) => Evidence::Unknown,
        }
    }

    fn attempts(
        &self,
        argv: &[String],
        cwd: &Path,
        withdraw: &dyn Fn() -> bool,
        call: &dyn Fn() -> io::Result<Option<ChildStatus>>,
    ) -> io::Result<Option<ChildStatus>> {
        let mut restarts = 0;
        loop {
            let Some(status) = call()? else {
                return Ok(None);
            };
            if restarts >= MAX_RESTARTS
                || !is_bazel(argv)
                || status.code != Some(RESTART_EXIT)
                || (self.policy.stop)()
                || !restartable(status.code, Self::evidence(argv, cwd))
            {
                return Ok(Some(status));
            }
            if withdraw() {
                return Ok(None);
            }
            restarts += 1;
            if let Some(path) = bep_path_of(argv) {
                let _ = std::fs::remove_file(cwd.join(path));
            }
            if !self.policy.pause(RESTART_EXIT, restarts) {
                return Ok(Some(status));
            }
            if withdraw() {
                return Ok(None);
            }
        }
    }
}

impl<R: Runner> Runner for RestartingRunner<R> {
    fn run(&self, argv: &[String], cwd: &Path, env: &[(&str, &str)]) -> io::Result<ChildStatus> {
        self.attempts(argv, cwd, &|| false, &|| {
            self.inner.run(argv, cwd, env).map(Some)
        })
        .map(|status| status.unwrap_or_else(|| ChildStatus::exited(RESTART_EXIT)))
    }

    fn run_hermetic(
        &self,
        argv: &[String],
        cwd: &Path,
        env: &[(&str, &str)],
    ) -> io::Result<ChildStatus> {
        self.inner.run_hermetic(argv, cwd, env)
    }

    fn run_withdrawable(
        &self,
        argv: &[String],
        cwd: &Path,
        env: &[(&str, &str)],
        withdraw: &dyn Fn() -> bool,
    ) -> io::Result<Option<ChildStatus>> {
        self.attempts(argv, cwd, withdraw, &|| {
            self.inner.run_withdrawable(argv, cwd, env, withdraw)
        })
    }

    fn run_app(
        &self,
        argv: &[String],
        cwd: &Path,
        env: &[(&str, &str)],
        withdraw: Option<&dyn Fn() -> bool>,
        held: &[&std::fs::File],
    ) -> io::Result<Option<ChildStatus>> {
        // Restarted clients may start a new Bazel server that would inherit the
        // handles, so only the first attempt carries them.
        let first = std::cell::Cell::new(true);
        self.attempts(argv, cwd, withdraw.unwrap_or(&|| false), &|| {
            let carried: &[&std::fs::File] = if first.replace(false) { held } else { &[] };
            self.inner.run_app(argv, cwd, env, withdraw, carried)
        })
    }

    fn pinned_tool(&self, var: &str) -> Option<PathBuf> {
        self.inner.pinned_tool(var)
    }

    fn tool_version(&self, path: &Path) -> Option<String> {
        self.inner.tool_version(path)
    }

    fn free_disk_bytes(&self, path: &Path) -> Option<u64> {
        self.inner.free_disk_bytes(path)
    }
}

/// A `QueryRunner` that restarts captured Bazel calls that crashed on startup.
pub struct RestartingQueryRunner<Q> {
    inner: Q,
    policy: Policy,
}

impl<Q: QueryRunner> RestartingQueryRunner<Q> {
    pub fn new(inner: Q) -> Self {
        RestartingQueryRunner {
            inner,
            policy: Policy::real(),
        }
    }

    #[cfg(test)]
    fn with_policy(inner: Q, policy: Policy) -> Self {
        RestartingQueryRunner { inner, policy }
    }

    fn attempts(
        &self,
        argv: &[String],
        call: &dyn Fn() -> io::Result<QueryResult>,
    ) -> io::Result<QueryResult> {
        let mut restarts = 0;
        loop {
            let result = call()?;
            if restarts >= MAX_RESTARTS
                || !is_bazel(argv)
                || (self.policy.stop)()
                || !restartable(result.code, Evidence::Stderr(&result.stderr))
            {
                return Ok(result);
            }
            restarts += 1;
            if !self.policy.pause(RESTART_EXIT, restarts) {
                return Ok(result);
            }
        }
    }
}

impl<Q: QueryRunner> QueryRunner for RestartingQueryRunner<Q> {
    fn run_query(&self, argv: &[String], cwd: &Path) -> io::Result<QueryResult> {
        self.attempts(argv, &|| self.inner.run_query(argv, cwd))
    }

    fn run_info(&self, argv: &[String], cwd: &Path) -> io::Result<QueryResult> {
        self.attempts(argv, &|| self.inner.run_info(argv, cwd))
    }

    fn free_disk_bytes(&self, path: &Path) -> Option<u64> {
        self.inner.free_disk_bytes(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};
    use std::collections::VecDeque;
    use std::rc::Rc;

    #[test]
    fn classifier_table() {
        use Evidence::*;
        assert!(restartable(Some(37), NotStarted));
        assert!(!restartable(Some(37), Started));
        assert!(!restartable(Some(37), Unknown));
        for code in [33, 36, 0, 1, 3, 8] {
            assert!(!restartable(Some(code), NotStarted), "{code}");
            assert!(!restartable(
                Some(code),
                Stderr(b"Server terminated abruptly")
            ));
        }
        assert!(!restartable(None, NotStarted));
        assert!(restartable(
            Some(37),
            Stderr(b"ERROR: could not acquire lock on repo contents cache: x")
        ));
        assert!(restartable(
            Some(37),
            Stderr(b"Server terminated abruptly (error code: 14, Socket closed)")
        ));
        assert!(!restartable(Some(37), Stderr(b"something else")));
        assert_eq!(backoff(1), Duration::from_secs(2));
        assert_eq!(backoff(2), Duration::from_secs(5));
    }

    #[test]
    fn finds_bep_path() {
        let argv: Vec<String> = ["bazel", "build", "--build_event_json_file=/x/bep.json"]
            .map(String::from)
            .to_vec();
        assert_eq!(bep_path_of(&argv), Some("/x/bep.json"));
        assert_eq!(bep_path_of(&argv[..2]), None);
    }

    struct Fake {
        codes: RefCell<VecDeque<i32>>,
        calls: Cell<u32>,
        write_started: Cell<bool>,
        bep: PathBuf,
        stale_seen: RefCell<Vec<bool>>,
    }

    impl Runner for Fake {
        fn run(&self, _: &[String], _: &Path, _: &[(&str, &str)]) -> io::Result<ChildStatus> {
            self.calls.set(self.calls.get() + 1);
            self.stale_seen.borrow_mut().push(self.bep.exists());
            if self.write_started.get() {
                std::fs::write(&self.bep, "{\"id\":{\"started\":{}},\"started\":{}}\n")?;
            } else {
                std::fs::write(&self.bep, "{\"id\":{\"started\":{}},\"aborted\":{}}\n")?;
            }
            let code = self.codes.borrow_mut().pop_front().unwrap_or(0);
            Ok(ChildStatus::exited(code))
        }
    }

    struct Shared(Rc<RefCell<Vec<u8>>>);

    impl Write for Shared {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.0.borrow_mut().extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    fn policy(stop: Rc<Cell<bool>>, out: Rc<RefCell<Vec<u8>>>) -> Policy {
        Policy {
            sleep: Box::new(|_| {}),
            stop: Box::new(move || stop.get()),
            notice: RefCell::new(Box::new(Shared(out))),
        }
    }

    struct Setup {
        dir: tempfile::TempDir,
        out: Rc<RefCell<Vec<u8>>>,
        stop: Rc<Cell<bool>>,
    }

    fn setup() -> Setup {
        Setup {
            dir: tempfile::tempdir().expect("tempdir"),
            out: Rc::default(),
            stop: Rc::default(),
        }
    }

    fn runner(s: &Setup, codes: &[i32], started: bool) -> RestartingRunner<Fake> {
        let fake = Fake {
            codes: RefCell::new(codes.iter().copied().collect()),
            calls: Cell::new(0),
            write_started: Cell::new(started),
            bep: s.dir.path().join("bep.json"),
            stale_seen: RefCell::default(),
        };
        RestartingRunner::with_policy(fake, policy(s.stop.clone(), s.out.clone()))
    }

    fn argv(bazel: &str, bep: bool) -> Vec<String> {
        let mut argv = vec![bazel.to_owned(), "build".to_owned()];
        if bep {
            argv.push("--build_event_json_file=bep.json".to_owned());
        }
        argv
    }

    struct Apps(RefCell<Vec<usize>>);

    impl Runner for Apps {
        fn run(&self, _: &[String], _: &Path, _: &[(&str, &str)]) -> io::Result<ChildStatus> {
            unreachable!("apps go through run_app")
        }

        fn run_app(
            &self,
            _: &[String],
            _: &Path,
            _: &[(&str, &str)],
            _: Option<&dyn Fn() -> bool>,
            held: &[&std::fs::File],
        ) -> io::Result<Option<ChildStatus>> {
            let mut seen = self.0.borrow_mut();
            seen.push(held.len());
            let code = if seen.len() == 1 { 37 } else { 0 };
            Ok(Some(ChildStatus::exited(code)))
        }
    }

    #[test]
    fn run_app_forwards_held_on_the_first_attempt_only() {
        let s = setup();
        let r = RestartingRunner::with_policy(
            Apps(RefCell::default()),
            policy(s.stop.clone(), s.out.clone()),
        );
        let file = tempfile::tempfile().expect("file");
        let status = r
            .run_app(&argv("bazel", true), s.dir.path(), &[], None, &[&file])
            .unwrap()
            .unwrap();
        assert_eq!(status.code, Some(0));
        assert_eq!(*r.inner.0.borrow(), vec![1, 0]);
    }

    #[test]
    fn restarts_once_then_succeeds() {
        let s = setup();
        let r = runner(&s, &[37, 0], false);
        let status = r.run(&argv("bazel", true), s.dir.path(), &[]).unwrap();
        assert_eq!(status.code, Some(0));
        assert_eq!(r.inner.calls.get(), 2);
        assert_eq!(*r.inner.stale_seen.borrow(), vec![false, false]);
        let text = String::from_utf8(s.out.borrow().clone()).unwrap();
        assert_eq!(text.lines().count(), 1);
        assert!(text.starts_with("dx: notice:"), "{text}");
    }

    #[test]
    fn gives_up_after_the_limit() {
        let s = setup();
        let r = runner(&s, &[37, 37, 37, 0], false);
        let status = r.run(&argv("bazel", true), s.dir.path(), &[]).unwrap();
        assert_eq!(status.code, Some(37));
        assert_eq!(r.inner.calls.get(), 3);
    }

    #[test]
    fn never_repeats_a_started_command() {
        let s = setup();
        let r = runner(&s, &[37, 0], true);
        let status = r.run(&argv("bazel", true), s.dir.path(), &[]).unwrap();
        assert_eq!(status.code, Some(37));
        assert_eq!(r.inner.calls.get(), 1);
    }

    #[test]
    fn passes_through_without_evidence_or_bazel() {
        let s = setup();
        let r = runner(&s, &[37, 0], false);
        assert_eq!(
            r.run(&argv("bazel", false), s.dir.path(), &[])
                .unwrap()
                .code,
            Some(37)
        );
        assert_eq!(r.inner.calls.get(), 1);
        let r = runner(&s, &[37, 0], false);
        assert_eq!(
            r.run(&argv("git", true), s.dir.path(), &[]).unwrap().code,
            Some(37)
        );
        assert_eq!(r.inner.calls.get(), 1);
    }

    #[test]
    fn stops_on_interrupt_or_withdraw() {
        let s = setup();
        s.stop.set(true);
        let r = runner(&s, &[37, 0], false);
        assert_eq!(
            r.run(&argv("bazel", true), s.dir.path(), &[]).unwrap().code,
            Some(37)
        );
        assert_eq!(r.inner.calls.get(), 1);

        let s = setup();
        let r = runner(&s, &[37, 0], false);
        let out = r
            .run_withdrawable(&argv("bazel", true), s.dir.path(), &[], &|| {
                r.inner.calls.get() >= 1
            })
            .unwrap();
        assert_eq!(out, None);
        assert_eq!(r.inner.calls.get(), 1);
    }

    struct FakeQuery {
        results: RefCell<VecDeque<(Option<i32>, &'static str)>>,
        calls: Cell<u32>,
    }

    impl QueryRunner for FakeQuery {
        fn run_query(&self, _: &[String], _: &Path) -> io::Result<QueryResult> {
            self.calls.set(self.calls.get() + 1);
            let (code, stderr) = self
                .results
                .borrow_mut()
                .pop_front()
                .unwrap_or((Some(0), ""));
            Ok(QueryResult {
                code,
                stdout: Vec::new(),
                stderr: stderr.as_bytes().to_vec(),
            })
        }
    }

    fn query(
        s: &Setup,
        results: &[(Option<i32>, &'static str)],
    ) -> RestartingQueryRunner<FakeQuery> {
        let fake = FakeQuery {
            results: RefCell::new(results.iter().copied().collect()),
            calls: Cell::new(0),
        };
        RestartingQueryRunner::with_policy(fake, policy(s.stop.clone(), s.out.clone()))
    }

    #[test]
    fn query_restarts_only_on_known_stderr() {
        let s = setup();
        let a = argv("bazel", false);
        let q = query(
            &s,
            &[
                (
                    Some(37),
                    "ERROR: could not acquire lock on repo contents cache",
                ),
                (Some(0), ""),
            ],
        );
        assert_eq!(q.run_query(&a, s.dir.path()).unwrap().code, Some(0));
        assert_eq!(q.inner.calls.get(), 2);

        let q = query(
            &s,
            &[
                (Some(37), "Server terminated abruptly (error code: 14)"),
                (Some(0), ""),
            ],
        );
        assert_eq!(q.run_info(&a, s.dir.path()).unwrap().code, Some(0));
        assert_eq!(q.inner.calls.get(), 2);

        let q = query(&s, &[(Some(37), "other"), (Some(0), "")]);
        assert_eq!(q.run_query(&a, s.dir.path()).unwrap().code, Some(37));
        assert_eq!(q.inner.calls.get(), 1);

        let q = query(&s, &[(None, "Server terminated abruptly"), (Some(0), "")]);
        assert_eq!(q.run_query(&a, s.dir.path()).unwrap().code, None);
        assert_eq!(q.inner.calls.get(), 1);
    }
}
