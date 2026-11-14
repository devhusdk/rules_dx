#![expect(
    unsafe_code,
    reason = "signal, process-group and pty tests call libc directly"
)]

use super::super::test_support::{closed_pipe, hang_up, reader_gone};
use std::ffi::{CStr, CString};
use std::fs;
use std::io;
#[cfg(target_os = "linux")]
use std::io::Write;
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

const MODULE: &str = "module(name = \"sample\", version = \"0.0.0\")\nbazel_dep(name = \"rules_dx\", version = \"0.0.0\")\n";

const FAKE_BAZEL: &str = r#"#!/bin/sh
case " $* " in *" --output_groups=dx_shards "*) exit 1 ;; esac
case " $* " in *" query "*|*" cquery "*|*" info "*)
  [ -n "$FAKE_QUERY_PROBE_ARGS" ] || exit 0
  echo "query $$" >> "$FAKE_BAZEL_LOG"
  exec "$FAKE_PROBE" $FAKE_QUERY_PROBE_ARGS ;;
esac
case " $* " in *" build "*|*" run "*|*" test "*) ;; *) exit 0 ;; esac
echo "start $$" >> "$FAKE_BAZEL_LOG"
exec "$FAKE_PROBE" $FAKE_PROBE_ARGS
"#;

struct Run {
    scratch: dx_test_scratch::TempDir,
    log: PathBuf,
    stdout: PathBuf,
    stderr: PathBuf,
    child: Child,
    started: Instant,
}

fn dx_binary() -> PathBuf {
    dx_testing::runfiles_root().join("cli/cli/dx")
}

const QUERY_WORDS: &[&str] = &["build", "pkg/a.py"];

fn dx_command(base: &Path, words: &[&str], probe_args: &str) -> Command {
    let mut command = dx_session(base, words, probe_args, "");
    command.process_group(0);
    command
}

fn dx_session(base: &Path, words: &[&str], probe_args: &str, query_args: &str) -> Command {
    let workspace = base.join("ws");
    let bin = base.join("bin");
    let tmp = base.join("tmp");
    for dir in [&workspace, &bin, &tmp, &workspace.join(".dx")] {
        fs::create_dir_all(dir).expect("probe dir");
    }
    if !workspace.join("MODULE.bazel").exists() {
        fs::write(workspace.join("MODULE.bazel"), MODULE).expect("module");
    }
    fs::write(workspace.join(".dx/pins"), "0.0.0\n").expect("pin");
    fs::write(workspace.join("BUILD.bazel"), "").expect("build file");
    fs::create_dir_all(workspace.join("pkg")).expect("pkg dir");
    fs::write(workspace.join("pkg/BUILD.bazel"), "").expect("pkg build file");
    fs::write(workspace.join("pkg/a.py"), "x = 1\n").expect("pkg source");
    let bazel = bin.join("bazel");
    dx_testing::executable_script(&bazel, FAKE_BAZEL);
    let log = base.join("bazel.log");
    fs::write(&log, "").expect("log");
    let mut command = Command::new(dx_binary());
    command
        .args(words)
        .current_dir(&workspace)
        .env_clear()
        .env("PATH", bin.as_os_str())
        .env("HOME", base)
        .env("DX_BUILD_SLOTS", "auto")
        .env("TMPDIR", &tmp)
        .env("FAKE_BAZEL_LOG", &log)
        .env("FAKE_PROBE", dx_testing::process_probe())
        .env("FAKE_PROBE_ARGS", probe_args)
        .env("FAKE_QUERY_PROBE_ARGS", query_args)
        .stdin(Stdio::null());
    command
}

struct Pty {
    #[cfg_attr(
        not(target_os = "linux"),
        expect(dead_code, reason = "only linux tests write to the master")
    )]
    master: fs::File,
    slave: CString,
}

/// `ptsname` returns one buffer shared by every thread.
static PTSNAME: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn open_pty() -> Pty {
    let _named = PTSNAME.lock().unwrap_or_else(|held| held.into_inner());
    // SAFETY: master is the descriptor just opened and owned once; ptsname is read under PTSNAME.
    unsafe {
        let master = libc::posix_openpt(libc::O_RDWR | libc::O_NOCTTY);
        assert!(master >= 0, "posix_openpt: {}", io::Error::last_os_error());
        let master = fs::File::from_raw_fd(master);
        assert_eq!(libc::grantpt(master.as_raw_fd()), 0, "grantpt");
        assert_eq!(libc::unlockpt(master.as_raw_fd()), 0, "unlockpt");
        let name = libc::ptsname(master.as_raw_fd());
        assert!(!name.is_null(), "ptsname");
        Pty {
            master,
            slave: CStr::from_ptr(name).to_owned(),
        }
    }
}

fn in_terminal(command: &mut Command, slave: CString) {
    // SAFETY: the pre_exec closure calls only async-signal-safe libc functions.
    unsafe {
        command.pre_exec(move || {
            if libc::setsid() < 0 {
                return Err(io::Error::last_os_error());
            }
            let tty = libc::open(slave.as_ptr(), libc::O_RDWR);
            if tty < 0 || libc::ioctl(tty, libc::TIOCSCTTY as _, 0) < 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }
}

impl Run {
    fn start(name: &str, words: &[&str], probe_args: &dyn Fn(&Path) -> String) -> Self {
        Self::launch(name, words, probe_args, None)
    }

    fn query(name: &str, words: &[&str], query_args: &dyn Fn(&Path) -> String) -> Self {
        let scratch = dx_test_scratch::scratch(name);
        let base = scratch.path().to_path_buf();
        let mut command = dx_session(&base, words, "", &query_args(&base));
        command.process_group(0);
        Self::spawn(scratch, command)
    }

    fn launch(
        name: &str,
        words: &[&str],
        probe_args: &dyn Fn(&Path) -> String,
        tty: Option<&Pty>,
    ) -> Self {
        let scratch = dx_test_scratch::scratch(name);
        let base = scratch.path().to_path_buf();
        let command = match tty {
            Some(pty) => {
                let mut command = dx_session(&base, words, &probe_args(&base), "");
                in_terminal(&mut command, pty.slave.clone());
                command
            }
            None => dx_command(&base, words, &probe_args(&base)),
        };
        Self::spawn(scratch, command)
    }

    fn spawn(scratch: dx_test_scratch::TempDir, mut command: Command) -> Self {
        let base = scratch.path().to_path_buf();
        let stdout = base.join("stdout");
        let stderr = base.join("stderr");
        let child = command
            .stdout(fs::File::create(&stdout).expect("stdout file"))
            .stderr(fs::File::create(&stderr).expect("stderr file"))
            .spawn()
            .expect("dx starts");
        Run {
            log: base.join("bazel.log"),
            scratch,
            stdout,
            stderr,
            child,
            started: Instant::now(),
        }
    }

    fn dir(&self) -> &Path {
        self.scratch.path()
    }

    fn stdout(&self) -> Vec<u8> {
        fs::read(&self.stdout).unwrap_or_default()
    }

    fn stderr(&self) -> Vec<u8> {
        fs::read(&self.stderr).unwrap_or_default()
    }

    fn context(&self) -> String {
        format!(
            "log:\n{}\nstderr:\n{}",
            fs::read_to_string(&self.log).unwrap_or_default(),
            String::from_utf8_lossy(&self.stderr()).replace('\0', "")
        )
    }

    fn build_pid(&mut self) -> i32 {
        self.logged_pid("start ")
    }

    fn query_pid(&mut self) -> i32 {
        self.logged_pid("query ")
    }

    fn logged_pid(&mut self, prefix: &str) -> i32 {
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            let log = fs::read_to_string(&self.log).unwrap_or_default();
            if let Some(pid) = log
                .lines()
                .find_map(|line| line.strip_prefix(prefix))
                .and_then(|pid| pid.trim().parse().ok())
            {
                return pid;
            }
            if let Ok(Some(status)) = self.child.try_wait() {
                panic!(
                    "dx exited ({status:?}) before Bazel logged {prefix:?}\n{}",
                    self.context()
                );
            }
            assert!(
                Instant::now() < deadline,
                "Bazel never logged {prefix:?}\n{}",
                self.context()
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    fn signal(&mut self, signo: i32) -> Instant {
        let pid = self.child.id() as i32;
        // SAFETY: kill takes plain integers and has no memory preconditions.
        assert_eq!(unsafe { libc::kill(pid, signo) }, 0, "signal delivered");
        Instant::now()
    }

    fn exit(&mut self) -> ExitStatus {
        let deadline = Instant::now() + Duration::from_secs(40);
        loop {
            if let Some(status) = self.child.try_wait().expect("wait") {
                return status;
            }
            if Instant::now() >= deadline {
                let _ = self.child.kill();
                let _ = self.child.wait();
                panic!("dx did not exit\n{}", self.context());
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

impl Drop for Run {
    fn drop(&mut self) {
        if let Ok(None) = self.child.try_wait() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

fn group_alive(pgid: i32) -> bool {
    // SAFETY: kill takes plain integers and has no memory preconditions.
    unsafe { libc::kill(-pgid, 0) == 0 }
}

fn kill_group(pgid: i32) {
    // SAFETY: kill takes plain integers and has no memory preconditions.
    unsafe {
        libc::kill(-pgid, libc::SIGKILL);
    }
}

fn json_events(stdout: &[u8]) -> Vec<serde_json::Value> {
    String::from_utf8_lossy(stdout)
        .lines()
        .map(|line| serde_json::from_str(line).unwrap_or_else(|_| panic!("not JSON: {line}")))
        .collect()
}

fn finished_exit(events: &[serde_json::Value]) -> Option<i64> {
    events
        .iter()
        .find(|event| event["event"] == "command_finished")
        .and_then(|event| event["exit_code"].as_i64())
}

fn run_dirs(run: &Run) -> Vec<PathBuf> {
    run_dirs_in(run.dir())
}

fn run_dirs_in(base: &Path) -> Vec<PathBuf> {
    fs::read_dir(base.join("tmp"))
        .expect("probe tmp")
        .map(|entry| entry.expect("tmp entry").path())
        .filter(|path| {
            path.file_name()
                .is_some_and(|name| name.to_string_lossy().starts_with("dx-run-"))
        })
        .collect()
}

fn zeros(bytes: &[u8]) -> usize {
    bytes.iter().filter(|byte| **byte == 0).count()
}

#[test]
fn large_streaming_output_reaches_stdout_in_text_and_stderr_in_json() {
    let probe = |_: &Path| "--stdout-bytes=4194304".to_owned();
    let mut text = Run::start("dx-stream-text-", &["build", "//..."], &probe);
    assert_eq!(text.exit().code(), Some(0), "{}", text.context());
    assert_eq!(zeros(&text.stdout()), 4_194_304);
    assert_eq!(zeros(&text.stderr()), 0);
    assert_eq!(run_dirs(&text), Vec::<PathBuf>::new());

    let mut json = Run::start(
        "dx-stream-json-",
        &["build", "//...", "--output=json"],
        &probe,
    );
    assert_eq!(json.exit().code(), Some(0), "{}", json.context());
    assert_eq!(zeros(&json.stderr()), 4_194_304);
    assert_eq!(finished_exit(&json_events(&json.stdout())), Some(0));
    assert_eq!(run_dirs(&json), Vec::<PathBuf>::new());
}

#[test]
fn a_failed_build_keeps_its_status_and_reports_it() {
    let probe = |_: &Path| "--stderr-text=boom --exit-code=3".to_owned();
    let mut text = Run::start("dx-stream-fail-", &["build", "//..."], &probe);
    assert_eq!(text.exit().code(), Some(3), "{}", text.context());
    assert!(String::from_utf8_lossy(&text.stderr()).contains("boom"));

    let mut json = Run::start(
        "dx-stream-fail-json-",
        &["build", "//...", "--output=json"],
        &probe,
    );
    assert_eq!(json.exit().code(), Some(3), "{}", json.context());
    let events = json_events(&json.stdout());
    assert!(
        events.iter().any(|event| event["code"] == "bazel_failed"),
        "{events:?}"
    );
    assert_eq!(finished_exit(&events), Some(3));
}

#[test]
fn a_closed_stdout_ends_the_run_promptly() {
    for (words, want) in [
        (&["build", "//..."][..], Some(141)),
        (&["build", "//...", "--output=json"][..], Some(141)),
    ] {
        let scratch = dx_test_scratch::scratch("dx-stream-pipe-");
        let writer = closed_pipe();
        let started = Instant::now();
        let status = dx_command(scratch.path(), words, "--stdout-bytes=8388608")
            .stdout(writer)
            .stderr(Stdio::null())
            .status()
            .expect("dx runs");
        assert!(
            started.elapsed() < Duration::from_secs(20),
            "{words:?}: {:?}",
            started.elapsed()
        );
        assert_eq!(status.code(), want, "{words:?}: {status:?}");
    }
}

#[test]
fn a_stop_signal_to_dx_alone_stops_the_build_and_its_descendants() {
    for signo in [libc::SIGINT, libc::SIGTERM, libc::SIGHUP] {
        for words in [
            &["build", "//..."][..],
            &["build", "//...", "--output=json"][..],
        ] {
            let mut run = Run::start("dx-stream-stop-", words, &|dir| {
                format!(
                    "--spawn-descendant --descendant-sleep-ms=30000 --descendant-marker={} --marker={}",
                    dir.join("descendant").display(),
                    dir.join("build").display()
                )
            });
            let build = run.build_pid();
            let deadline = Instant::now() + Duration::from_secs(10);
            while !(group_alive(build) && run_dirs(&run).len() == 1) && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(20));
            }
            assert!(group_alive(build), "{}", run.context());
            assert_eq!(run_dirs(&run).len(), 1, "{}", run.context());
            let sent = run.signal(signo);
            let status = run.exit();
            let elapsed = sent.elapsed();
            let alive = group_alive(build);
            kill_group(build);
            assert_eq!(
                status.signal(),
                Some(signo),
                "{signo} {words:?}: {status:?}\n{}",
                run.context()
            );
            assert!(elapsed < Duration::from_secs(5), "{signo}: {elapsed:?}");
            assert!(!alive, "{signo} {words:?}: owned processes survived");
            assert!(!run.dir().join("descendant").exists());
            assert!(!run.dir().join("build").exists());
            assert_eq!(
                run_dirs(&run),
                Vec::<PathBuf>::new(),
                "{signo} {words:?}: the run temp dir must be removed"
            );
            if words.contains(&"--output=json") {
                assert_eq!(finished_exit(&json_events(&run.stdout())), None);
            }
        }
    }
}

#[test]
fn a_stop_signal_to_dx_alone_stops_a_hanging_query_and_its_descendants() {
    for signo in [libc::SIGINT, libc::SIGTERM, libc::SIGHUP] {
        let mut run = Run::query("dx-stream-query-stop-", QUERY_WORDS, &|dir| {
            format!(
                "--spawn-descendant --descendant-sleep-ms=30000 --descendant-alive-lock={} --descendant-marker={} --marker={}",
                dir.join("descendant.alive").display(),
                dir.join("descendant").display(),
                dir.join("query").display()
            )
        });
        let query = run.query_pid();
        let alive = run.dir().join("descendant.alive");
        wait_for_file(&mut run, &alive);
        assert!(group_alive(query), "{}", run.context());
        assert_eq!(
            // SAFETY: getpgid takes a plain pid and has no memory preconditions.
            unsafe { libc::getpgid(query) },
            query,
            "the query must lead its own group\n{}",
            run.context()
        );
        let sent = run.signal(signo);
        let status = run.exit();
        let elapsed = sent.elapsed();
        let alive = group_alive(query);
        kill_group(query);
        assert_eq!(
            status.signal(),
            Some(signo),
            "{signo}: {status:?}\n{}",
            run.context()
        );
        assert!(elapsed < Duration::from_secs(5), "{signo}: {elapsed:?}");
        assert!(!alive, "{signo}: the query group survived");
        assert!(!run.dir().join("descendant").exists());
        assert!(!run.dir().join("query").exists());
        assert_eq!(
            run_dirs(&run),
            Vec::<PathBuf>::new(),
            "{signo}: the run temp dir must be removed"
        );
        assert_eq!(bazel_starts(run.dir()), 0, "{}", run.context());
    }
}

fn wait_for_file(run: &mut Run, path: &Path) {
    let deadline = Instant::now() + Duration::from_secs(30);
    while !path.exists() {
        if let Ok(Some(status)) = run.child.try_wait() {
            panic!("dx exited ({status:?}) early\n{}", run.context());
        }
        assert!(
            Instant::now() < deadline,
            "{} never appeared",
            path.display()
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// The group numbers recorded in the leases the running dx holds.
fn recorded_groups(run: &Run) -> Vec<i32> {
    let prefix = format!("{}-", run.child.id());
    fs::read_dir(run.dir().join("ws/.dx/local/leases/process-groups"))
        .into_iter()
        .flatten()
        .flatten()
        .filter(|entry| entry.file_name().to_string_lossy().starts_with(&prefix))
        .filter_map(|entry| fs::read_to_string(entry.path()).ok())
        .filter_map(|text| text.split_whitespace().next()?.parse().ok())
        .collect()
}

/// Waits until dx holds a written lease for `pgid`; on Linux it must already be written.
fn wait_for_lease(run: &mut Run, pgid: i32) {
    let deadline = Instant::now() + Duration::from_secs(30);
    while !recorded_groups(run).contains(&pgid) {
        if cfg!(target_os = "linux") {
            panic!(
                "no lease records group {pgid} while it runs\n{}",
                run.context()
            );
        }
        if let Ok(Some(status)) = run.child.try_wait() {
            panic!("dx exited ({status:?}) early\n{}", run.context());
        }
        assert!(
            Instant::now() < deadline,
            "no lease ever recorded group {pgid}\n{}",
            run.context()
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Waits until dx has reaped `pid`.
fn wait_until_reaped(run: &mut Run, pid: i32) {
    let deadline = Instant::now() + Duration::from_secs(30);
    // SAFETY: kill takes plain integers and has no memory preconditions.
    while unsafe { libc::kill(pid, 0) } == 0 {
        if let Ok(Some(status)) = run.child.try_wait() {
            panic!("dx exited ({status:?}) early\n{}", run.context());
        }
        assert!(Instant::now() < deadline, "{pid} never exited");
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Waits until job control has stopped `pid`.
#[cfg(target_os = "linux")]
fn wait_until_stopped(pid: i32) {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let stat = fs::read_to_string(format!("/proc/{pid}/stat")).unwrap_or_default();
        let state = stat.rsplit_once(") ").map(|(_, rest)| rest.chars().next());
        if state == Some(Some('T')) {
            return;
        }
        assert!(Instant::now() < deadline, "{pid} never stopped: {stat}");
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[cfg(target_os = "linux")]
#[derive(Debug, Clone, Copy)]
enum Sender {
    Tty,
    Kill,
    GroupKill,
    BuildGroup,
    Unattributed,
}

#[cfg(target_os = "linux")]
fn queue_unattributed(pid: i32, signo: i32) {
    // SAFETY: siginfo_t is plain C data for which all-zero is valid.
    let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
    info.si_signo = signo;
    info.si_code = libc::SI_QUEUE;
    // SAFETY: info outlives the call and is the siginfo_t that rt_sigqueueinfo reads.
    let rc = unsafe { libc::syscall(libc::SYS_rt_sigqueueinfo, pid, signo, &info) };
    assert_eq!(rc, 0, "rt_sigqueueinfo: {}", io::Error::last_os_error());
}

#[cfg(target_os = "linux")]
fn a_terminal_child_gets_sigint_once(sender: Sender) {
    let pty = open_pty();
    let mut run = Run::launch(
        "dx-stream-tty-",
        &["build", "//..."],
        &|dir| {
            format!(
                "--detached-descendant --descendant-count-sigint={} --descendant-sleep-ms=30000 --sleep-ms=30000",
                dir.join("sigint").display()
            )
        },
        Some(&pty),
    );
    let build = run.build_pid();
    let log = run.dir().join("sigint");
    wait_for_file(&mut run, &log);
    let build_pgid = assert_foreground_build(&run, &pty, build);
    match sender {
        Sender::Tty => (&pty.master).write_all(b"\x03").expect("ctrl-c"),
        Sender::Kill => {
            run.signal(libc::SIGINT);
        }
        // SAFETY: kill takes plain integers and has no memory preconditions.
        Sender::GroupKill => unsafe {
            libc::kill(-(run.child.id() as i32), libc::SIGINT);
        },
        Sender::BuildGroup => unsafe {
            libc::kill(-build_pgid, libc::SIGINT);
        },
        #[cfg(target_os = "linux")]
        Sender::Unattributed => queue_unattributed(run.child.id() as i32, libc::SIGINT),
    }
    let expected = b"I";
    let status = run.exit();
    let counted = fs::read(&log).expect("sigint log");
    // SAFETY: kill takes plain integers and has no memory preconditions.
    unsafe {
        libc::kill(-build_pgid, libc::SIGKILL);
    }
    assert!(
        wait_until_gone(build_pgid, Duration::from_secs(5)),
        "{sender:?}: the terminal group survived SIGKILL"
    );
    assert_eq!(
        status.signal(),
        Some(libc::SIGINT),
        "{sender:?}: {status:?}\n{}",
        run.context()
    );
    assert_eq!(
        counted,
        expected,
        "{sender:?}: wrong SIGINT count\n{}",
        run.context()
    );
    assert_eq!(run_dirs(&run), Vec::<PathBuf>::new(), "{sender:?}");
}

#[cfg(target_os = "linux")]
#[test]
fn a_terminal_child_gets_sigint_once_from_tty() {
    a_terminal_child_gets_sigint_once(Sender::Tty);
}

#[cfg(target_os = "linux")]
#[test]
fn a_terminal_child_gets_sigint_once_from_kill() {
    a_terminal_child_gets_sigint_once(Sender::Kill);
}

#[cfg(target_os = "linux")]
#[test]
fn a_terminal_child_gets_sigint_once_from_group_kill() {
    a_terminal_child_gets_sigint_once(Sender::GroupKill);
}

#[cfg(target_os = "linux")]
#[test]
fn a_terminal_child_gets_sigint_once_from_build_group() {
    a_terminal_child_gets_sigint_once(Sender::BuildGroup);
}

#[cfg(target_os = "linux")]
#[test]
fn a_terminal_child_gets_sigint_once_from_unattributed() {
    a_terminal_child_gets_sigint_once(Sender::Unattributed);
}

#[cfg(target_os = "linux")]
fn assert_foreground_build(run: &Run, pty: &Pty, build: i32) -> i32 {
    // SAFETY: getpgid takes a plain pid and has no memory preconditions.
    let build_pgid = unsafe { libc::getpgid(build) };
    assert_ne!(
        build_pgid,
        run.child.id() as i32,
        "the build must leave dx's group\n{}",
        run.context()
    );
    assert_eq!(
        build_pgid,
        // SAFETY: tcgetpgrp reads the pty master descriptor, which stays open for the call.
        unsafe { libc::tcgetpgrp(pty.master.as_raw_fd()) },
        "the build's group must hold the terminal\n{}",
        run.context()
    );
    build_pgid
}

#[cfg(target_os = "linux")]
#[test]
fn a_terminal_build_resumes_after_ctrl_z() {
    let pty = open_pty();
    let mut run = Run::launch(
        "dx-stream-tty-stop-",
        &["build", "//..."],
        &|dir| {
            format!(
                "--count-sigint={} --gate={} --sleep-ms=30000",
                dir.join("sigint").display(),
                dir.join("gate").display()
            )
        },
        Some(&pty),
    );
    let build = run.build_pid();
    let log = run.dir().join("sigint");
    wait_for_file(&mut run, &log);
    let build_pgid = assert_foreground_build(&run, &pty, build);
    let dx = run.child.id() as i32;
    let mut held = 0;
    // SAFETY: kill and waitpid take plain integers; held is a valid status out-pointer.
    unsafe {
        libc::kill(dx, libc::SIGSTOP);
        assert_eq!(libc::waitpid(dx, &mut held, libc::WUNTRACED), dx);
    }
    assert!(libc::WIFSTOPPED(held), "{held}\n{}", run.context());
    (&pty.master).write_all(b"\x1a").expect("ctrl-z");
    wait_until_stopped(build);
    fs::write(run.dir().join("gate"), b"").expect("open the gate");
    // SAFETY: kill takes plain integers and has no memory preconditions.
    unsafe {
        libc::kill(dx, libc::SIGCONT);
    }
    let status = run.exit();
    assert_eq!(status.code(), Some(0), "{status:?}\n{}", run.context());
    assert!(wait_until_gone(build_pgid, Duration::from_secs(5)));
}

#[test]
fn the_terminal_returns_to_dx_after_a_build() {
    let pty = open_pty();
    let scratch = dx_test_scratch::scratch("dx-stream-tty-return-");
    let base = scratch.path().to_path_buf();
    let session = dx_session(&base, &["build", "//..."], "", "");
    let script = format!(
        "{} build //... && exec {} --require-foreground",
        dx_binary().display(),
        dx_testing::process_probe().display()
    );
    let mut command = Command::new("/bin/sh");
    command.arg("-c").arg(script).stdin(Stdio::null());
    command.current_dir(session.get_current_dir().expect("workspace"));
    command.env_clear();
    for (key, value) in session.get_envs() {
        if let Some(value) = value {
            command.env(key, value);
        }
    }
    in_terminal(&mut command, pty.slave.clone());
    let mut run = Run::spawn(scratch, command);
    let status = run.exit();
    assert_eq!(status.code(), Some(0), "{status:?}\n{}", run.context());
}

fn wait_until_gone(pgid: i32, within: Duration) -> bool {
    let deadline = Instant::now() + within;
    while group_alive(pgid) {
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    true
}

#[cfg(target_os = "linux")]
#[test]
fn killing_dx_stops_the_test_it_started() {
    let mut run = Run::start("dx-stream-kill-", &["test", "//..."], &|dir| {
        format!(
            "--sleep-ms=30000 --alive-lock={} --marker={}",
            dir.join("build.alive").display(),
            dir.join("build").display()
        )
    });
    let build = run.build_pid();
    let alive = run.dir().join("build.alive");
    wait_for_file(&mut run, &alive);
    assert!(group_alive(build), "{}", run.context());
    run.signal(libc::SIGKILL);
    let status = run.exit();
    let gone = wait_until_gone(build, Duration::from_secs(10));
    kill_group(build);
    assert_eq!(status.signal(), Some(libc::SIGKILL), "{status:?}");
    assert!(gone, "the test survived a killed dx");
    assert!(!run.dir().join("build").exists());
}

#[cfg(target_os = "linux")]
#[test]
fn killing_dx_stops_the_query_it_started() {
    let mut run = Run::query("dx-stream-query-kill-", QUERY_WORDS, &|dir| {
        format!(
            "--sleep-ms=30000 --alive-lock={} --marker={}",
            dir.join("query.alive").display(),
            dir.join("query").display()
        )
    });
    let query = run.query_pid();
    let alive = run.dir().join("query.alive");
    wait_for_file(&mut run, &alive);
    assert!(group_alive(query), "{}", run.context());
    run.signal(libc::SIGKILL);
    let status = run.exit();
    let gone = wait_until_gone(query, Duration::from_secs(10));
    kill_group(query);
    assert_eq!(status.signal(), Some(libc::SIGKILL), "{status:?}");
    assert!(gone, "the query survived a killed dx");
    assert!(!run.dir().join("query").exists());
}

#[test]
fn the_next_run_stops_what_a_killed_dx_left_behind_from_a_query() {
    for json in [false, true] {
        let mut words = QUERY_WORDS.to_vec();
        if json {
            words.push("--output=json");
        }
        let mut run = Run::query("dx-stream-query-leftover-", &words, &|dir| {
            format!(
                "--spawn-descendant --descendant-sleep-ms=30000 --descendant-alive-lock={} --descendant-marker={}",
                dir.join("descendant.alive").display(),
                dir.join("descendant").display()
            )
        });
        let query = run.query_pid();
        let alive = run.dir().join("descendant.alive");
        wait_for_file(&mut run, &alive);
        wait_for_lease(&mut run, query);
        run.signal(libc::SIGKILL);
        let _ = run.exit();
        assert!(group_alive(query), "the descendant should outlive dx");
        let mut next_words = vec!["build", "//..."];
        if json {
            next_words.push("--output=json");
        }
        let next = dx_command(run.dir(), &next_words, "")
            .output()
            .expect("next dx runs");
        let gone = wait_until_gone(query, Duration::from_secs(10));
        kill_group(query);
        assert_eq!(next.status.code(), Some(0), "{next:?}");
        assert!(
            gone,
            "json={json}: leftovers survived the next run\n{}\nreaped.log:\n{}",
            String::from_utf8_lossy(&next.stderr),
            fs::read_to_string(
                run.dir()
                    .join("ws/.dx/local/leases/process-groups/reaped.log")
            )
            .unwrap_or_default()
        );
        let stderr = String::from_utf8_lossy(&next.stderr);
        assert!(
            stderr.contains("stopped leftover process group"),
            "{stderr}"
        );
        if json {
            let events = json_events(&next.stdout);
            assert!(
                events
                    .iter()
                    .any(|event| event["code"] == "orphan_process_group_stopped"),
                "{events:?}"
            );
            assert_eq!(finished_exit(&events), Some(0));
        }
        assert!(!run.dir().join("descendant").exists());
    }
}

#[test]
fn the_next_run_stops_what_a_killed_dx_left_behind() {
    for words in [
        &["test", "//..."][..],
        &["test", "//...", "--output=json"][..],
    ] {
        let mut run = Run::start("dx-stream-leftover-", words, &|dir| {
            format!(
                "--spawn-descendant --descendant-sleep-ms=30000 --descendant-alive-lock={} --descendant-marker={}",
                dir.join("descendant.alive").display(),
                dir.join("descendant").display()
            )
        });
        let build = run.build_pid();
        let alive = run.dir().join("descendant.alive");
        wait_for_file(&mut run, &alive);
        run.signal(libc::SIGKILL);
        let _ = run.exit();
        let leftover_dirs = run_dirs(&run);
        assert_eq!(leftover_dirs.len(), 1, "{leftover_dirs:?}");
        assert!(group_alive(build), "the descendant should outlive dx");
        let next_words: Vec<&str> = words
            .iter()
            .map(|word| if *word == "test" { "build" } else { word })
            .collect();
        let next = dx_command(run.dir(), &next_words, "")
            .output()
            .expect("next dx runs");
        let gone = wait_until_gone(build, Duration::from_secs(10));
        kill_group(build);
        assert_eq!(next.status.code(), Some(0), "{next:?}");
        assert!(
            gone,
            "{words:?}: leftovers survived the next run\n{}\nreaped.log:\n{}",
            String::from_utf8_lossy(&next.stderr),
            fs::read_to_string(
                run.dir()
                    .join("ws/.dx/local/leases/process-groups/reaped.log")
            )
            .unwrap_or_default()
        );
        for dir in &leftover_dirs {
            assert!(!dir.exists(), "{} must be swept", dir.display());
        }
        let stderr = String::from_utf8_lossy(&next.stderr);
        assert!(
            stderr.contains("stopped leftover process group"),
            "{stderr}"
        );
        if words.contains(&"--output=json") {
            let events = json_events(&next.stdout);
            assert_eq!(events[0]["event"], "command_started", "{events:?}");
            assert!(
                events
                    .iter()
                    .skip(1)
                    .any(|event| event["code"] == "orphan_process_group_stopped"),
                "{events:?}"
            );
            assert_eq!(finished_exit(&events), Some(0));
        }
        let quiet = dx_command(run.dir(), &next_words, "")
            .output()
            .expect("quiet dx runs");
        assert!(
            !String::from_utf8_lossy(&quiet.stderr).contains("leftover"),
            "a second run must find nothing"
        );
    }
}

#[test]
fn a_stop_after_bazel_exits_reaps_the_descendant_holding_stdout() {
    let mut run = Run::start(
        "dx-stream-linger-",
        &["build", "//...", "--output=json"],
        &|dir| {
            format!(
                "--stdout-text=early --detached-descendant --descendant-sleep-ms=30000 --descendant-alive-lock={} --descendant-marker={}",
                dir.join("descendant.alive").display(),
                dir.join("descendant").display()
            )
        },
    );
    let build = run.build_pid();
    let alive = run.dir().join("descendant.alive");
    wait_for_file(&mut run, &alive);
    wait_until_reaped(&mut run, build);
    let sent = run.signal(libc::SIGTERM);
    let status = run.exit();
    let elapsed = sent.elapsed();
    let alive = group_alive(build);
    kill_group(build);
    assert_eq!(status.signal(), Some(libc::SIGTERM), "{}", run.context());
    assert!(elapsed < Duration::from_secs(5), "{elapsed:?}");
    assert!(!alive, "the descendant holding stdout survived");
    assert!(String::from_utf8_lossy(&run.stderr()).contains("early"));
    assert_eq!(finished_exit(&json_events(&run.stdout())), None);
}

#[test]
fn a_descendant_holding_stdout_does_not_hold_dx() {
    let mut run = Run::start(
        "dx-stream-held-",
        &["build", "//...", "--output=json"],
        &|_| "--stdout-text=early --detached-descendant --descendant-sleep-ms=30000".to_owned(),
    );
    let build = run.build_pid();
    let status = run.exit();
    let elapsed = run.started.elapsed();
    kill_group(build);
    assert_eq!(status.code(), Some(0), "{}", run.context());
    assert!(elapsed < Duration::from_secs(15), "{elapsed:?}");
    assert!(String::from_utf8_lossy(&run.stderr()).contains("early"));
    assert_eq!(finished_exit(&json_events(&run.stdout())), Some(0));
}

#[test]
fn stopping_one_worktree_leaves_another_running() {
    let mut stopped = Run::start("dx-stream-one-", &["build", "//..."], &|dir| {
        format!("--sleep-ms=30000 --marker={}", dir.join("build").display())
    });
    let mut kept = Run::start("dx-stream-two-", &["build", "//..."], &|dir| {
        format!(
            "--gate={} --sleep-ms=30000 --marker={}",
            dir.join("gate").display(),
            dir.join("build").display()
        )
    });
    let stopped_build = stopped.build_pid();
    let kept_build = kept.build_pid();
    stopped.signal(libc::SIGTERM);
    let status = stopped.exit();
    let stopped_alive = group_alive(stopped_build);
    kill_group(stopped_build);
    assert_eq!(
        status.signal(),
        Some(libc::SIGTERM),
        "{}",
        stopped.context()
    );
    assert!(!stopped_alive);
    assert!(
        group_alive(kept_build),
        "the other worktree's build was hit"
    );
    fs::write(kept.dir().join("gate"), b"").expect("open the gate");
    assert_eq!(kept.exit().code(), Some(0), "{}", kept.context());
    assert!(kept.dir().join("build").exists());
    assert!(!stopped.dir().join("build").exists());
}

const GATED_BAZEL: &str = r#"#!/bin/sh
case " $* " in *" --output_groups=dx_shards "*) exit 1 ;; esac
case " $* " in *" build "*|*" run "*) ;; *) exit 0 ;; esac
echo "start $$" >> "$FAKE_BAZEL_LOG"
while [ ! -e "$FAKE_GATE" ]; do sleep 0.05; done
exit "${FAKE_EXIT:-0}"
"#;

fn gated_command(base: &Path, exit: i32) -> (Command, PathBuf) {
    gated_words(base, exit, &["build", "//...", "--output=json"])
}

fn gated_words(base: &Path, exit: i32, words: &[&str]) -> (Command, PathBuf) {
    let mut command = dx_command(base, words, "");
    let bazel = base.join("bin/bazel");
    dx_testing::executable_script(&bazel, GATED_BAZEL);
    let gate = base.join("gate");
    command
        .env("FAKE_GATE", &gate)
        .env("FAKE_EXIT", exit.to_string());
    (command, gate)
}

fn bazel_starts(base: &Path) -> usize {
    fs::read_to_string(base.join("bazel.log"))
        .unwrap_or_default()
        .lines()
        .filter(|line| line.starts_with("start "))
        .count()
}

#[test]
fn hang_up_waits_for_every_reader_copy() {
    let (reader, writer) = io::pipe().expect("pipe");
    let mut holder = Command::new("sleep")
        .arg("60")
        .stdin(reader.try_clone().expect("reader clone"))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("sleep starts");
    let (done, finished) = std::sync::mpsc::channel();
    let waiter = std::thread::spawn(move || {
        hang_up(reader, &writer);
        done.send(writer).expect("send");
    });
    assert!(
        finished.recv_timeout(Duration::from_millis(300)).is_err(),
        "hang_up returned while the child held the read end"
    );
    holder.kill().expect("kill sleep");
    holder.wait().expect("sleep exits");
    let writer = finished
        .recv_timeout(Duration::from_secs(30))
        .expect("hang_up returns once the child is gone");
    waiter.join().expect("waiter");
    assert!(reader_gone(&writer, 0));
}

#[test]
fn a_stdout_closed_before_the_start_event_starts_no_bazel() {
    let scratch = dx_test_scratch::scratch("dx-stream-nostart-");
    let writer = closed_pipe();
    let (mut command, gate) = gated_command(scratch.path(), 0);
    fs::write(gate, "").expect("gate open");
    let status = command
        .stdout(writer)
        .stderr(Stdio::null())
        .status()
        .expect("dx runs");
    assert_eq!(status.code(), Some(141), "{status:?}");
    assert_eq!(bazel_starts(scratch.path()), 0);
}

#[test]
fn an_ignored_hangup_does_not_stop_dx() {
    let scratch = dx_test_scratch::scratch("dx-stream-nohup-");
    let (mut command, gate) = gated_command(scratch.path(), 0);
    // SAFETY: the pre_exec closure calls only signal, which is async-signal-safe.
    unsafe {
        command.pre_exec(|| {
            libc::signal(libc::SIGHUP, libc::SIG_IGN);
            Ok(())
        });
    }
    let mut child = command
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("dx starts");
    let deadline = Instant::now() + Duration::from_secs(30);
    while bazel_starts(scratch.path()) == 0 && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(bazel_starts(scratch.path()), 1);
    // SAFETY: kill takes plain integers and has no memory preconditions.
    unsafe {
        libc::kill(child.id() as i32, libc::SIGHUP);
    }
    assert!(child.try_wait().expect("poll").is_none(), "dx stopped");
    fs::write(&gate, "").expect("gate open");
    let status = child.wait().expect("dx exits");
    assert_eq!(status.code(), Some(0), "{status:?}");
}

#[test]
fn a_stdout_closed_after_the_start_event_fails_the_completion() {
    use std::io::Read;
    for (child_exit, want) in [(0, 141), (3, 3)] {
        let scratch = dx_test_scratch::scratch("dx-stream-closed-");
        let (mut reader, writer) = std::io::pipe().expect("pipe");
        let probe = writer.try_clone().expect("pipe clone");
        let (mut command, gate) = gated_command(scratch.path(), child_exit);
        let stderr = scratch.path().join("stderr");
        let mut child = command
            .stdout(writer)
            .stderr(fs::File::create(&stderr).expect("stderr file"))
            .spawn()
            .expect("dx starts");
        drop(command);
        let mut received = Vec::new();
        let mut byte = [0u8; 1];
        while !received.ends_with(b"\n") {
            reader.read_exact(&mut byte).expect("start event arrives");
            received.push(byte[0]);
        }
        hang_up(reader, &probe);
        drop(probe);
        fs::write(&gate, "").expect("gate open");
        let status = child.wait().expect("dx exits");
        let events = json_events(&received);
        assert_eq!(events.len(), 1, "{events:?}");
        assert_eq!(events[0]["event"], "command_started");
        assert_eq!(status.code(), Some(want), "{status:?}");
        assert_eq!(bazel_starts(scratch.path()), 1);
        if child_exit != 0 {
            let text = fs::read_to_string(&stderr).unwrap_or_default();
            assert!(text.contains("keeping exit 3"), "{text}");
        }
    }
}

#[cfg(target_os = "linux")]
#[test]
fn a_failing_destination_is_an_operational_failure() {
    let scratch = dx_test_scratch::scratch("dx-stream-full-");
    let (mut command, gate) = gated_command(scratch.path(), 0);
    fs::write(gate, "").expect("gate open");
    let status = command
        .stdout(
            fs::OpenOptions::new()
                .write(true)
                .open("/dev/full")
                .expect("/dev/full"),
        )
        .stderr(Stdio::null())
        .status()
        .expect("dx runs");
    assert_eq!(status.code(), Some(1), "{status:?}");
    assert_eq!(bazel_starts(scratch.path()), 0);
}

const QUALITY_WORDS: [&[&str]; 2] = [
    &["lint", "//...", "--apply", "--output=json"],
    &["format", "//...", "--apply", "--output=json"],
];

fn write_source(base: &Path) -> PathBuf {
    let source = base.join("ws/a.py");
    fs::write(&source, "x = 1\n").expect("source");
    source
}

#[test]
fn quality_with_stdout_closed_before_the_start_event_starts_nothing() {
    for words in QUALITY_WORDS {
        let scratch = dx_test_scratch::scratch("dx-quality-nostart-");
        let writer = closed_pipe();
        let (mut command, gate) = gated_words(scratch.path(), 0, words);
        let source = write_source(scratch.path());
        fs::write(gate, "").expect("gate open");
        let status = command
            .stdout(writer)
            .stderr(Stdio::null())
            .status()
            .expect("dx runs");
        assert_eq!(status.code(), Some(141), "{words:?} {status:?}");
        assert_eq!(bazel_starts(scratch.path()), 0, "{words:?}");
        assert_eq!(fs::read_to_string(source).expect("source"), "x = 1\n");
    }
}

#[test]
fn quality_with_stdout_closed_after_the_start_event_ends_with_141() {
    use std::io::Read;
    for words in QUALITY_WORDS {
        let scratch = dx_test_scratch::scratch("dx-quality-closed-");
        let (mut reader, writer) = std::io::pipe().expect("pipe");
        let probe = writer.try_clone().expect("pipe clone");
        let (mut command, gate) = gated_words(scratch.path(), 0, words);
        let source = write_source(scratch.path());
        let mut child = command
            .stdout(writer)
            .stderr(Stdio::null())
            .spawn()
            .expect("dx starts");
        drop(command);
        let mut received = Vec::new();
        let mut byte = [0u8; 1];
        while !received.ends_with(b"\n") {
            reader.read_exact(&mut byte).expect("start event arrives");
            received.push(byte[0]);
        }
        hang_up(reader, &probe);
        drop(probe);
        fs::write(&gate, "").expect("gate open");
        let status = child.wait().expect("dx exits");
        let events = json_events(&received);
        assert_eq!(events.len(), 1, "{words:?} {events:?}");
        assert_eq!(events[0]["event"], "command_started");
        assert_eq!(status.code(), Some(141), "{words:?} {status:?}");
        assert_eq!(bazel_starts(scratch.path()), 1);
        assert_eq!(fs::read_to_string(source).expect("source"), "x = 1\n");
    }
}

#[cfg(target_os = "linux")]
#[test]
fn quality_with_a_failing_destination_starts_nothing() {
    for words in QUALITY_WORDS {
        let scratch = dx_test_scratch::scratch("dx-quality-full-");
        let (mut command, gate) = gated_words(scratch.path(), 0, words);
        let source = write_source(scratch.path());
        fs::write(gate, "").expect("gate open");
        let status = command
            .stdout(
                fs::OpenOptions::new()
                    .write(true)
                    .open("/dev/full")
                    .expect("/dev/full"),
            )
            .stderr(Stdio::null())
            .status()
            .expect("dx runs");
        assert_eq!(status.code(), Some(1), "{words:?} {status:?}");
        assert_eq!(bazel_starts(scratch.path()), 0, "{words:?}");
        assert_eq!(fs::read_to_string(source).expect("source"), "x = 1\n");
    }
}

const DELIVERY_WORDS: [&[&str]; 2] = [
    &["generate", "--apply", "--output=json"],
    &["status", "--output=json"],
];

#[test]
fn generate_and_adoption_with_stdout_closed_start_nothing() {
    for words in DELIVERY_WORDS {
        let scratch = dx_test_scratch::scratch("dx-delivery-closed-");
        let writer = closed_pipe();
        let (mut command, gate) = gated_words(scratch.path(), 0, words);
        fs::write(gate, "").expect("gate open");
        let status = command
            .stdout(writer)
            .stderr(Stdio::null())
            .status()
            .expect("dx runs");
        assert_eq!(status.code(), Some(141), "{words:?} {status:?}");
        assert_eq!(bazel_starts(scratch.path()), 0, "{words:?}");
    }
}

#[cfg(target_os = "linux")]
#[test]
fn generate_and_adoption_with_a_failing_destination_start_nothing() {
    for words in DELIVERY_WORDS {
        let scratch = dx_test_scratch::scratch("dx-delivery-full-");
        let (mut command, gate) = gated_words(scratch.path(), 0, words);
        fs::write(gate, "").expect("gate open");
        let status = command
            .stdout(
                fs::OpenOptions::new()
                    .write(true)
                    .open("/dev/full")
                    .expect("/dev/full"),
            )
            .stderr(Stdio::null())
            .status()
            .expect("dx runs");
        assert_eq!(status.code(), Some(1), "{words:?} {status:?}");
        assert_eq!(bazel_starts(scratch.path()), 0, "{words:?}");
    }
}

fn hold_all_slots(base: &Path) -> Vec<dx_plan::slots::Slot> {
    let slots = base.join(".cache/dx/slots");
    fs::create_dir_all(&slots).expect("slot dir");
    (0..dx_plan::slots::MAX_SLOTS)
        .map(|_| {
            dx_plan::slots::acquire(&slots, dx_plan::slots::MAX_SLOTS, "other-tree", &mut |_| {})
                .expect("slot")
        })
        .collect()
}

fn spawn_waiting(base: &Path, words: &[&str]) -> (Child, PathBuf, PathBuf) {
    let stdout = base.join("stdout");
    let stderr = base.join("stderr");
    let mut child = dx_command(base, words, "")
        .stdout(fs::File::create(&stdout).expect("stdout file"))
        .stderr(fs::File::create(&stderr).expect("stderr file"))
        .spawn()
        .expect("dx starts");
    let deadline = Instant::now() + Duration::from_secs(30);
    while !fs::read_to_string(&stderr)
        .unwrap_or_default()
        .contains("waiting for a build slot")
    {
        if Instant::now() >= deadline || matches!(child.try_wait(), Ok(Some(_))) {
            let _ = child.kill();
            let _ = child.wait();
            panic!(
                "dx never waited for a slot {words:?}\n{}",
                fs::read_to_string(&stderr).unwrap_or_default()
            );
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    (child, stdout, stderr)
}

fn write_dependency_sets(base: &Path) {
    let workspace = base.join("ws");
    fs::create_dir_all(workspace.join("app")).expect("app dir");
    fs::write(
        workspace.join("app/Cargo.toml"),
        "[package]\nname = \"app\"\nversion = \"0.1.0\"\n",
    )
    .expect("manifest");
    fs::write(workspace.join("app/Cargo.lock"), "version = 4\n").expect("lock");
    fs::write(workspace.join("app/cargo-bazel-lock.json"), "{}\n").expect("bazel lock");
    fs::write(
        workspace.join("MODULE.bazel"),
        MODULE.to_owned()
            + "crate = use_extension(\"@rules_rust//crate_universe:extensions.bzl\", \"crate\")\ncrate.from_cargo(name = \"crates\", cargo_lockfile = \"//app:Cargo.lock\", lockfile = \"//app:cargo-bazel-lock.json\", manifests = [\"//app:Cargo.toml\"])\nuse_repo(crate, \"crates\")\n",
    )
    .expect("module");
}

#[test]
fn a_build_waiting_for_a_slot_starts_with_command_started() {
    let scratch = dx_test_scratch::scratch("dx-stream-slot-");
    let base = scratch.path();
    let held = hold_all_slots(base);
    let (mut child, stdout, _) = spawn_waiting(base, &["build", "//...", "--output=json"]);
    drop(held);
    let status = child.wait().expect("dx exits");
    let events = json_events(&fs::read(&stdout).expect("stdout"));
    assert_eq!(status.code(), Some(0), "{events:?}");
    assert_eq!(events[0]["event"], "command_started", "{events:?}");
    assert!(
        events[1..]
            .iter()
            .any(|event| event["code"] == "waiting_for_build_slot"),
        "{events:?}"
    );
    assert_eq!(finished_exit(&events), Some(0));
}

#[test]
fn a_stop_signal_during_the_slot_wait_cancels_with_128_plus_signal() {
    let text: &[&str] = &["build", "//..."];
    let json: &[&str] = &["build", "//...", "--output=json"];
    let run_text: &[&str] = &["run", "//demo:app"];
    let run_json: &[&str] = &["run", "//demo:app", "--output=json"];
    let sets_text: &[&str] = &["update", "--apply", "crates"];
    let sets_json: &[&str] = &["update", "--apply", "crates", "--output=json"];
    for signo in [libc::SIGINT, libc::SIGTERM] {
        for (words, configured) in [
            (text, false),
            (json, false),
            (run_text, false),
            (run_json, false),
            (sets_text, true),
            (sets_json, true),
        ] {
            let scratch = dx_test_scratch::scratch("dx-stream-slot-stop-");
            let base = scratch.path();
            if configured {
                write_dependency_sets(base);
            }
            let held = hold_all_slots(base);
            let (mut child, stdout, stderr) = spawn_waiting(base, words);
            let pid = i32::try_from(child.id()).expect("pid");
            // SAFETY: kill takes plain integers and has no memory preconditions.
            assert_eq!(unsafe { libc::kill(pid, signo) }, 0, "{signo} {words:?}");
            let deadline = Instant::now() + Duration::from_secs(10);
            let status = loop {
                if let Some(status) = child.try_wait().expect("try_wait") {
                    break status;
                }
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    panic!("dx ignored signal {signo} {words:?}");
                }
                std::thread::sleep(Duration::from_millis(10));
            };
            let err = fs::read_to_string(&stderr).expect("stderr");
            let out = fs::read(&stdout).expect("stdout");
            assert_eq!(
                status.code(),
                Some(128 + signo),
                "{signo} {words:?} {status:?}\n{err}"
            );
            assert!(
                err.contains(&format!("dx: cancelled: stopped by signal {signo}")),
                "{signo} {words:?}\n{err}"
            );
            if words.contains(&"--output=json") {
                let events = json_events(&out);
                assert_eq!(
                    finished_exit(&events),
                    Some(i64::from(128 + signo)),
                    "{signo} {words:?} {events:?}"
                );
                let finished = events
                    .iter()
                    .filter(|event| event["event"] == "command_finished")
                    .count();
                assert_eq!(finished, 1, "{signo} {words:?} {events:?}");
                let error = events
                    .iter()
                    .position(|event| event["event"] == "error" && event["code"] == "cancelled")
                    .unwrap_or_else(|| panic!("{signo} {words:?} {events:?}"));
                if let Some(notice) = events
                    .iter()
                    .position(|event| event["code"] == "waiting_for_build_slot")
                {
                    assert!(notice < error, "{signo} {words:?} {events:?}");
                }
            }
            assert_eq!(bazel_starts(base), 0, "{signo} {words:?}");
            assert_eq!(
                run_dirs_in(base),
                Vec::<PathBuf>::new(),
                "{signo} {words:?}"
            );
            drop(held);
        }
    }
}
