use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

const MODULE: &str = "module(name = \"sample\", version = \"0.0.0\")\nbazel_dep(name = \"rules_dx\", version = \"0.0.0\")\n";

const FAKE_BAZEL: &str = r#"#!/bin/sh
case " $* " in *" build "*) ;; *) exit 0 ;; esac
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

fn dx_command(base: &Path, words: &[&str], probe_args: &str) -> Command {
    let workspace = base.join("ws");
    let bin = base.join("bin");
    let tmp = base.join("tmp");
    for dir in [&workspace, &bin, &tmp, &workspace.join(".dx")] {
        fs::create_dir_all(dir).expect("probe dir");
    }
    fs::write(workspace.join("MODULE.bazel"), MODULE).expect("module");
    fs::write(workspace.join(".dx/version"), "0.0.0\n").expect("pin");
    fs::write(workspace.join("BUILD.bazel"), "").expect("build file");
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
        .stdin(Stdio::null())
        .process_group(0);
    command
}

impl Run {
    fn start(name: &str, words: &[&str], probe_args: &dyn Fn(&Path) -> String) -> Self {
        let scratch = dx_test_scratch::scratch(name);
        let base = scratch.path().to_path_buf();
        let stdout = base.join("stdout");
        let stderr = base.join("stderr");
        let child = dx_command(&base, words, &probe_args(&base))
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
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            let log = fs::read_to_string(&self.log).unwrap_or_default();
            if let Some(pid) = log
                .lines()
                .find_map(|line| line.strip_prefix("start "))
                .and_then(|pid| pid.trim().parse().ok())
            {
                return pid;
            }
            if let Ok(Some(status)) = self.child.try_wait() {
                panic!("dx exited ({status:?}) before Bazel\n{}", self.context());
            }
            assert!(
                Instant::now() < deadline,
                "Bazel never started\n{}",
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

    let mut json = Run::start(
        "dx-stream-json-",
        &["build", "//...", "--output=json"],
        &probe,
    );
    assert_eq!(json.exit().code(), Some(0), "{}", json.context());
    assert_eq!(zeros(&json.stderr()), 4_194_304);
    assert_eq!(finished_exit(&json_events(&json.stdout())), Some(0));
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
        (&["build", "//...", "--output=json"][..], Some(0)),
    ] {
        let scratch = dx_test_scratch::scratch("dx-stream-pipe-");
        let (reader, writer) = std::io::pipe().expect("pipe");
        drop(reader);
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
    for signo in [libc::SIGINT, libc::SIGTERM] {
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
            if words.contains(&"--output=json") {
                assert_eq!(finished_exit(&json_events(&run.stdout())), None);
            }
        }
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
        format!("--sleep-ms=2000 --marker={}", dir.join("build").display())
    });
    let stopped_build = stopped.build_pid();
    let kept_build = kept.build_pid();
    std::thread::sleep(Duration::from_millis(300));
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
    assert_eq!(kept.exit().code(), Some(0), "{}", kept.context());
    assert!(kept.dir().join("build").exists());
    assert!(!stopped.dir().join("build").exists());
}
