use std::ffi::{CStr, CString};
use std::fs;
use std::io::{self, Write};
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::fs::PermissionsExt;
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
    fs::write(workspace.join("MODULE.bazel"), MODULE).expect("module");
    fs::write(workspace.join(".dx/version"), "0.0.0\n").expect("pin");
    fs::write(workspace.join("BUILD.bazel"), "").expect("build file");
    fs::create_dir_all(workspace.join("pkg")).expect("pkg dir");
    fs::write(workspace.join("pkg/BUILD.bazel"), "").expect("pkg build file");
    fs::write(workspace.join("pkg/a.py"), "x = 1\n").expect("pkg source");
    let bazel = bin.join("bazel");
    fs::write(&bazel, FAKE_BAZEL).expect("fake bazel");
    fs::set_permissions(&bazel, fs::Permissions::from_mode(0o755)).expect("chmod");
    let log = base.join("bazel.log");
    fs::write(&log, "").expect("log");
    let mut command = Command::new(dx_binary());
    command
        .args(words)
        .current_dir(&workspace)
        .env_clear()
        .env("PATH", format!("{}:/usr/bin:/bin", bin.display()))
        .env("HOME", base)
        .env("TMPDIR", &tmp)
        .env("FAKE_BAZEL_LOG", &log)
        .env("FAKE_PROBE", dx_testing::process_probe())
        .env("FAKE_PROBE_ARGS", probe_args)
        .env("FAKE_QUERY_PROBE_ARGS", query_args)
        .stdin(Stdio::null());
    command
}

struct Pty {
    master: fs::File,
    slave: CString,
}

fn open_pty() -> Pty {
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
    unsafe { libc::kill(-pgid, 0) == 0 }
}

fn kill_group(pgid: i32) {
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
            std::thread::sleep(Duration::from_millis(300));
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
                "--spawn-descendant --descendant-sleep-ms=30000 --descendant-marker={} --marker={}",
                dir.join("descendant").display(),
                dir.join("query").display()
            )
        });
        let query = run.query_pid();
        std::thread::sleep(Duration::from_millis(300));
        assert!(group_alive(query), "{}", run.context());
        assert_eq!(
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

#[derive(Debug, Clone, Copy)]
enum Sender {
    Tty,
    Kill,
    #[cfg(target_os = "linux")]
    Unattributed,
}

fn senders() -> Vec<Sender> {
    vec![
        Sender::Tty,
        Sender::Kill,
        #[cfg(target_os = "linux")]
        Sender::Unattributed,
    ]
}

fn expected_sigints(sender: Sender) -> &'static [u8] {
    match sender {
        Sender::Kill if cfg!(not(target_os = "linux")) => b"",
        _ => b"I",
    }
}

#[cfg(target_os = "linux")]
fn queue_unattributed(pid: i32, signo: i32) {
    let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
    info.si_signo = signo;
    info.si_code = libc::SI_QUEUE;
    let rc = unsafe { libc::syscall(libc::SYS_rt_sigqueueinfo, pid, signo, &info) };
    assert_eq!(rc, 0, "rt_sigqueueinfo: {}", io::Error::last_os_error());
}

#[test]
fn a_terminal_child_gets_each_sigint_once() {
    for sender in senders() {
        let pty = open_pty();
        let mut run = Run::launch(
            "dx-stream-tty-",
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
        assert_eq!(
            unsafe { libc::getpgid(build) },
            run.child.id() as i32,
            "the build must share the terminal group\n{}",
            run.context()
        );
        match sender {
            Sender::Tty => (&pty.master).write_all(b"\x03").expect("ctrl-c"),
            Sender::Kill => {
                run.signal(libc::SIGINT);
            }
            #[cfg(target_os = "linux")]
            Sender::Unattributed => queue_unattributed(run.child.id() as i32, libc::SIGINT),
        }
        let expected = expected_sigints(sender);
        let deadline = Instant::now() + Duration::from_secs(5);
        while fs::read(&log).map_or(0, |bytes| bytes.len()) < expected.len()
            && Instant::now() < deadline
        {
            std::thread::sleep(Duration::from_millis(10));
        }
        if expected.is_empty() {
            std::thread::sleep(Duration::from_millis(100));
        }
        fs::write(run.dir().join("gate"), b"").expect("open the gate");
        let status = run.exit();
        assert_eq!(
            status.signal(),
            Some(libc::SIGINT),
            "{sender:?}: {status:?}\n{}",
            run.context()
        );
        assert_eq!(
            fs::read(&log).expect("sigint log"),
            expected,
            "{sender:?}: wrong SIGINT count\n{}",
            run.context()
        );
        assert_eq!(run_dirs(&run), Vec::<PathBuf>::new(), "{sender:?}");
    }
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
        format!("--sleep-ms=30000 --marker={}", dir.join("build").display())
    });
    let build = run.build_pid();
    std::thread::sleep(Duration::from_millis(300));
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
        format!("--sleep-ms=30000 --marker={}", dir.join("query").display())
    });
    let query = run.query_pid();
    std::thread::sleep(Duration::from_millis(300));
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
                "--spawn-descendant --descendant-sleep-ms=30000 --descendant-marker={}",
                dir.join("descendant").display()
            )
        });
        let query = run.query_pid();
        std::thread::sleep(Duration::from_millis(300));
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
        assert!(gone, "json={json}: leftovers survived the next run");
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
                "--spawn-descendant --descendant-sleep-ms=30000 --descendant-marker={}",
                dir.join("descendant").display()
            )
        });
        let build = run.build_pid();
        std::thread::sleep(Duration::from_millis(300));
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
        assert!(gone, "{words:?}: leftovers survived the next run");
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
                "--stdout-text=early --detached-descendant --descendant-sleep-ms=30000 --descendant-marker={}",
                dir.join("descendant").display()
            )
        },
    );
    let build = run.build_pid();
    std::thread::sleep(Duration::from_millis(500));
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
    fs::write(&bazel, GATED_BAZEL).expect("gated bazel");
    fs::set_permissions(&bazel, fs::Permissions::from_mode(0o755)).expect("chmod");
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

/// Reports whether the pipe behind `writer` has no reader left in any process.
fn reader_gone(writer: &io::PipeWriter, timeout_ms: i32) -> bool {
    let mut fd = libc::pollfd {
        fd: writer.as_raw_fd(),
        events: 0,
        revents: 0,
    };
    let ready = unsafe { libc::poll(&mut fd, 1, timeout_ms) };
    ready > 0 && fd.revents & (libc::POLLERR | libc::POLLHUP) != 0
}

/// Drops `reader` and waits until no process holds a copy of the read end.
fn hang_up(reader: io::PipeReader, writer: &io::PipeWriter) {
    drop(reader);
    let deadline = Instant::now() + Duration::from_secs(30);
    while !reader_gone(writer, 1000) {
        assert!(Instant::now() < deadline, "pipe reader still held");
    }
}

/// Returns the write end of a pipe whose read end is closed everywhere.
fn closed_pipe() -> io::PipeWriter {
    let (reader, writer) = io::pipe().expect("pipe");
    hang_up(reader, &writer);
    writer
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
    unsafe {
        libc::kill(child.id() as i32, libc::SIGHUP);
    }
    std::thread::sleep(Duration::from_millis(300));
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

fn hold_all_slots(base: &Path) -> Vec<dx_cli_core::slots::Slot> {
    let slots = base.join(".cache/dx/slots");
    fs::create_dir_all(&slots).expect("slot dir");
    (0..dx_cli_core::slots::MAX_SLOTS)
        .map(|_| {
            dx_cli_core::slots::acquire(
                &slots,
                dx_cli_core::slots::MAX_SLOTS,
                "other-tree",
                &mut |_| {},
            )
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
    for signo in [libc::SIGINT, libc::SIGTERM] {
        for words in [text, json] {
            let scratch = dx_test_scratch::scratch("dx-stream-slot-stop-");
            let base = scratch.path();
            let held = hold_all_slots(base);
            let (mut child, stdout, stderr) = spawn_waiting(base, words);
            let pid = i32::try_from(child.id()).expect("pid");
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
            if words == json {
                let events = json_events(&out);
                assert_eq!(finished_exit(&events), None, "{signo} {words:?} {events:?}");
                assert!(
                    events
                        .iter()
                        .all(|event| event["event"] != "command_finished"),
                    "{signo} {words:?} {events:?}"
                );
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
