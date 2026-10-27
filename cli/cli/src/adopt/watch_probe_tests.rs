use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

const MODULE: &str = "module(name = \"sample\", version = \"0.0.0\")\nbazel_dep(name = \"rules_dx\", version = \"0.0.0\")\n";

const FAKE_BAZEL: &str = r#"#!/bin/sh
case " $* " in *" build "*|*" run "*) ;; *) exit 0 ;; esac
log="$FAKE_BAZEL_LOG"
for pid in $(sed -nE 's/^(start|child) ([0-9]+).*/\2/p' "$log"); do
  kill -0 "$pid" 2>/dev/null && echo "overlap $pid" >> "$log"
done
echo "start $$ $*" >> "$log"
n=$(grep -c '^start ' "$log")
trap 'echo "interrupted $$" >> "$log"; kill "$child" 2>/dev/null; wait "$child"; exit 8' INT TERM
pause="${FAKE_BAZEL_SLEEP:-0}"
case " $* " in *" build "*) pause="${FAKE_BAZEL_BUILD_SLEEP:-$pause}" ;; esac
sleep "$pause" &
child=$!
echo "child $child" >> "$log"
wait "$child"
for path in $FAKE_BAZEL_TOUCH; do
  mkdir -p "$(dirname "$path")"
  echo "$n" > "$path"
done
code=$(echo "$FAKE_BAZEL_CODES" | cut -d' ' -f"$n")
echo "end $$ ${code:-0}" >> "$log"
exit "${code:-0}"
"#;

struct Probe {
    _scratch: dx_test_scratch::TempDir,
    workspace: PathBuf,
    log: PathBuf,
    stdout: PathBuf,
    stderr: PathBuf,
    child: Child,
    signalled: Option<Instant>,
    exited: Option<Duration>,
}

fn dx_binary() -> PathBuf {
    let root = std::env::var("TEST_SRCDIR").expect("TEST_SRCDIR is set under Bazel");
    let workspace = std::env::var("TEST_WORKSPACE").expect("TEST_WORKSPACE is set under Bazel");
    Path::new(&root).join(workspace).join("cli/cli/dx")
}

impl Probe {
    fn disk_cache_flag(&self) -> String {
        let home = self.workspace.parent().expect("probe base");
        let cache = dx_process::default_disk_cache(&home.join(".cache"));
        format!("--disk_cache={}", cache.display())
    }

    fn start(name: &str, words: &[&str], fake: &[(&str, &str)]) -> Self {
        let scratch = dx_test_scratch::scratch(name);
        let base = scratch.path().to_path_buf();
        let workspace = base.join("ws");
        let bin = base.join("bin");
        let tmp = base.join("tmp");
        for dir in [&workspace, &bin, &tmp, &workspace.join(".dx")] {
            fs::create_dir_all(dir).expect("probe dir");
        }
        fs::write(workspace.join("MODULE.bazel"), MODULE).expect("module");
        fs::write(workspace.join(".dx/version"), "0.0.0\n").expect("pin");
        fs::write(workspace.join("BUILD.bazel"), "").expect("build file");
        fs::create_dir_all(workspace.join("src")).expect("src");
        fs::write(workspace.join("src/main.rs"), "fn main() {}\n").expect("source");
        fs::create_dir_all(workspace.join("app")).expect("app");
        fs::write(workspace.join("app/BUILD.bazel"), "").expect("app build file");
        let bazel = bin.join("bazel");
        fs::write(&bazel, FAKE_BAZEL).expect("fake bazel");
        fs::set_permissions(&bazel, fs::Permissions::from_mode(0o755)).expect("chmod");
        let log = base.join("bazel.log");
        fs::write(&log, "").expect("log");
        let stdout = base.join("stdout");
        let stderr = base.join("stderr");
        let mut command = Command::new(dx_binary());
        command
            .args(words)
            .current_dir(&workspace)
            .env_clear()
            .env("PATH", format!("{}:/usr/bin:/bin", bin.display()))
            .env("HOME", &base)
            .env("TMPDIR", &tmp)
            .env("FAKE_BAZEL_LOG", &log)
            .stdin(Stdio::null())
            .stdout(fs::File::create(&stdout).expect("stdout file"))
            .stderr(fs::File::create(&stderr).expect("stderr file"))
            .process_group(0);
        for (key, value) in fake {
            command.env(key, value);
        }
        let child = command.spawn().expect("dx starts");
        Probe {
            _scratch: scratch,
            workspace,
            log,
            stdout,
            stderr,
            child,
            signalled: None,
            exited: None,
        }
    }

    fn log(&self) -> String {
        fs::read_to_string(&self.log).unwrap_or_default()
    }

    fn count(&self, prefix: &str) -> usize {
        self.log()
            .lines()
            .filter(|line| line.starts_with(prefix))
            .count()
    }

    fn stdout(&self) -> String {
        fs::read_to_string(&self.stdout).unwrap_or_default()
    }

    fn stderr(&self) -> String {
        fs::read_to_string(&self.stderr).unwrap_or_default()
    }

    fn until(&mut self, what: &str, done: impl Fn(&Probe) -> bool) {
        let deadline = Instant::now() + Duration::from_secs(30);
        while !done(self) {
            if let Ok(Some(status)) = self.child.try_wait() {
                panic!(
                    "dx exited ({status:?}) before {what}\nlog:\n{}\nstdout:\n{}\nstderr:\n{}",
                    self.log(),
                    self.stdout(),
                    self.stderr()
                );
            }
            assert!(
                Instant::now() < deadline,
                "timed out waiting for {what}\nlog:\n{}\nstdout:\n{}\nstderr:\n{}",
                self.log(),
                self.stdout(),
                self.stderr()
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    fn idle(&mut self, ends: usize) {
        self.until("the iteration to finish", |probe| {
            probe.count("end ") >= ends && probe.stdout().contains("watching (Ctrl-C")
        });
        std::thread::sleep(Duration::from_millis(400));
    }

    fn settle(&self) {
        std::thread::sleep(Duration::from_millis(1200));
    }

    fn signal(&mut self, signo: i32, group: bool) {
        self.signalled = Some(Instant::now());
        let pid = self.child.id() as i32;
        let target = if group { -pid } else { pid };
        assert_eq!(unsafe { libc::kill(target, signo) }, 0, "signal delivered");
    }

    fn exit(&mut self) -> ExitStatus {
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            if let Some(status) = self.child.try_wait().expect("wait") {
                self.exited = self.signalled.map(|at| at.elapsed());
                return status;
            }
            if Instant::now() >= deadline {
                let _ = self.child.kill();
                let _ = self.child.wait();
                panic!(
                    "dx did not exit\nlog:\n{}\nstdout:\n{}\nstderr:\n{}",
                    self.log(),
                    self.stdout(),
                    self.stderr()
                );
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    fn exit_time(&self) -> String {
        format!("{:?} from signal to exit", self.exited)
    }

    fn stop(&mut self) -> ExitStatus {
        self.signal(libc::SIGINT, false);
        self.exit()
    }
}

impl Drop for Probe {
    fn drop(&mut self) {
        if let Ok(None) = self.child.try_wait() {
            unsafe {
                libc::kill(-(self.child.id() as i32), libc::SIGKILL);
            }
            let _ = self.child.wait();
        }
    }
}

fn alive(pid: i32) -> bool {
    unsafe { libc::kill(pid, 0) == 0 && !zombie(pid) }
}

#[cfg(target_os = "linux")]
fn zombie(pid: i32) -> bool {
    fs::read_to_string(format!("/proc/{pid}/stat"))
        .ok()
        .and_then(|stat| {
            stat.rsplit_once(')')
                .and_then(|(_, rest)| rest.trim_start().chars().next())
        })
        .is_some_and(|state| state == 'Z' || state == 'X')
}

#[cfg(not(target_os = "linux"))]
fn zombie(_pid: i32) -> bool {
    false
}

fn starts_before_previous_end(log: &str) -> bool {
    let mut running = false;
    for line in log.lines() {
        if line.starts_with("start ") {
            if running {
                return true;
            }
            running = true;
        } else if line.starts_with("end ") || line.starts_with("interrupted ") {
            running = false;
        }
    }
    false
}

fn command_args<'a>(line: &'a str, verb: &str) -> Vec<&'a str> {
    line.split_whitespace()
        .skip_while(|word| *word != verb)
        .skip(1)
        .collect()
}

fn has_cache_and_settings(args: &[&str], cache: &str) -> bool {
    args.first() == Some(&cache)
        && dx_plan::plan::BAZEL_SETTINGS
            .iter()
            .all(|setting| args.contains(setting))
}

#[test]
fn edits_during_a_slow_command_queue_exactly_one_more_iteration() {
    let mut probe = Probe::start(
        "dx-watch-probe-slow-",
        &["watch", "build", "//...", "--", "--jobs=4", "--apply"],
        &[("FAKE_BAZEL_SLEEP", "1.5")],
    );
    probe.until("the first command", |probe| probe.count("child ") == 1);
    for name in ["one.rs", "two.rs", "three.rs"] {
        fs::write(probe.workspace.join("src").join(name), "edit").expect("burst edit");
    }
    probe.idle(2);
    probe.settle();
    let log = probe.log();
    assert_eq!(probe.count("start "), 2, "one pending iteration:\n{log}");
    assert!(!starts_before_previous_end(&log), "no overlap:\n{log}");
    let cache = probe.disk_cache_flag();
    for line in log.lines().filter(|line| line.starts_with("start ")) {
        let args = command_args(line, "build");
        let bep = args
            .iter()
            .filter(|arg| arg.starts_with("--build_event_json_file="))
            .collect::<Vec<_>>();
        assert!(
            has_cache_and_settings(&args, &cache)
                && args.windows(2).any(|pair| pair == ["--jobs=4", "--apply"])
                && args.iter().filter(|arg| **arg == "--jobs=4").count() == 1
                && args.iter().filter(|arg| **arg == "--apply").count() == 1
                && bep.len() == 1
                && bep[0].ends_with(".json")
                && args.last() == Some(&"//..."),
            "the -- payload reaches Bazel unchanged: {line}"
        );
    }
    let status = probe.stop();
    assert_eq!(status.code(), Some(130), "{}", probe.stderr());
    let stdout = probe.stdout();
    assert_eq!(
        stdout.matches("changed ").count(),
        1,
        "the burst is one trigger: {stdout}"
    );
    assert!(stdout.ends_with("stopped\n"), "{stdout}");
}

#[test]
fn idle_edits_renames_deletes_and_config_changes_each_rerun() {
    let mut probe = Probe::start("dx-watch-probe-idle-", &["watch", "build", "//..."], &[]);
    probe.idle(1);
    let workspace = probe.workspace.clone();
    let steps: [(&str, Box<dyn Fn()>); 5] = [
        (
            "edit",
            Box::new(|| fs::write(workspace.join("src/main.rs"), "fn main() { }\n").expect("edit")),
        ),
        (
            "rename",
            Box::new(|| {
                fs::rename(workspace.join("src/main.rs"), workspace.join("src/app.rs"))
                    .expect("rename")
            }),
        ),
        (
            "delete",
            Box::new(|| fs::remove_file(workspace.join("src/app.rs")).expect("delete")),
        ),
        (
            "local config",
            Box::new(|| fs::write(workspace.join("dx.local.toml"), "[dx]\n").expect("local")),
        ),
        (
            "committed config",
            Box::new(|| fs::write(workspace.join("dx.toml"), "[dx]\n").expect("committed")),
        ),
    ];
    for (round, (what, step)) in steps.iter().enumerate() {
        step();
        let want = round + 2;
        probe.until(what, |probe| probe.count("end ") >= want);
        probe.idle(want);
    }
    probe.settle();
    assert_eq!(probe.count("start "), 6, "{}", probe.log());
    assert_eq!(probe.stop().code(), Some(130));
}

#[test]
fn generated_outputs_written_by_the_command_never_loop() {
    let mut probe = Probe::start(
        "dx-watch-probe-loop-",
        &["watch", "build", "//..."],
        &[("FAKE_BAZEL_TOUCH", "bazel-out/k8/bin/app .dx/state")],
    );
    probe.idle(1);
    probe.settle();
    assert_eq!(probe.count("start "), 1, "{}", probe.log());
    assert!(probe.workspace.join("bazel-out/k8/bin/app").is_file());
    assert_eq!(probe.stop().code(), Some(130));
}

#[test]
fn a_failed_iteration_keeps_watching_and_recovers() {
    let mut probe = Probe::start(
        "dx-watch-probe-recover-",
        &["watch", "build", "//..."],
        &[("FAKE_BAZEL_CODES", "3 0")],
    );
    probe.idle(1);
    assert!(probe.log().contains("end ") && probe.log().contains(" 3\n"));
    fs::write(probe.workspace.join("src/main.rs"), "fixed").expect("fix");
    probe.idle(2);
    assert_eq!(probe.count("start "), 2, "{}", probe.log());
    assert!(probe.log().lines().last().unwrap_or("").ends_with(" 0"));
    assert_eq!(probe.stop().code(), Some(130));
}

#[test]
fn removing_the_workspace_is_an_observer_failure() {
    let mut probe = Probe::start(
        "dx-watch-probe-observer-",
        &["watch", "build", "//..."],
        &[],
    );
    probe.idle(1);
    fs::remove_dir_all(&probe.workspace).expect("remove workspace");
    let status = probe.exit();
    assert_eq!(status.code(), Some(1), "{}", probe.stderr());
    assert!(
        probe
            .stderr()
            .contains("watch_failed: watch failed: watch root removed"),
        "{}",
        probe.stderr()
    );
    assert_eq!(probe.count("start "), 1);
}

#[test]
fn idle_terminate_stops_the_watch() {
    let mut probe = Probe::start("dx-watch-probe-term-", &["watch", "build", "//..."], &[]);
    probe.idle(1);
    probe.signal(libc::SIGTERM, false);
    assert_eq!(probe.exit().code(), Some(143));
    assert_eq!(probe.count("start "), 1);
}

#[test]
fn ctrl_c_during_a_command_stops_it_its_descendants_and_the_watch() {
    for group in [true, false] {
        let mut probe = Probe::start(
            "dx-watch-probe-ctrl-c-",
            &["watch", "build", "//..."],
            &[("FAKE_BAZEL_SLEEP", "30")],
        );
        probe.until("the command's descendant", |probe| {
            probe.count("child ") == 1
        });
        let descendant: i32 = probe
            .log()
            .lines()
            .find_map(|line| line.strip_prefix("child "))
            .and_then(|pid| pid.trim().parse().ok())
            .expect("descendant pid");
        assert!(alive(descendant));
        probe.signal(libc::SIGINT, group);
        let status = probe.exit();
        assert!(
            status.code() == Some(130) || status.signal() == Some(libc::SIGINT),
            "group={group}: {status:?} {}\n{}",
            probe.exit_time(),
            probe.stderr()
        );
        assert!(
            probe.count("interrupted ") >= 1,
            "group={group}: dx exited before the trap ran\nlog:\n{}",
            probe.log()
        );
        assert!(
            !alive(descendant),
            "group={group}: descendant {descendant} survived dx exit\nlog:\n{}",
            probe.log()
        );
        std::thread::sleep(Duration::from_millis(400));
        assert_eq!(probe.count("start "), 1, "no launch after the signal");
    }
}

#[test]
fn json_watch_keeps_every_stdout_line_an_event() {
    let mut probe = Probe::start(
        "dx-watch-probe-json-",
        &["watch", "build", "//...", "--output=json"],
        &[],
    );
    probe.until("the first iteration", |probe| {
        probe.count("end ") == 1 && probe.stderr().contains("watching (Ctrl-C")
    });
    std::thread::sleep(Duration::from_millis(400));
    fs::write(probe.workspace.join("src/main.rs"), "edit").expect("edit");
    probe.until("the second iteration", |probe| {
        probe.stdout().matches("\"command_finished\"").count() == 2
    });
    assert_eq!(probe.stop().code(), Some(130));
    let stdout = probe.stdout();
    let events: Vec<serde_json::Value> = stdout
        .lines()
        .map(|line| serde_json::from_str(line).unwrap_or_else(|_| panic!("not JSON: {line}")))
        .collect();
    assert_eq!(
        events
            .iter()
            .filter(|event| event["event"] == "command_started")
            .count(),
        2,
        "{stdout}"
    );
    let stderr = probe.stderr();
    for needle in ["iteration=1", "changed ", "iteration=2", "stopped"] {
        assert!(stderr.contains(needle), "{needle}: {stderr}");
    }
}

fn descendants(probe: &Probe) -> Vec<i32> {
    probe
        .log()
        .lines()
        .filter_map(|line| line.strip_prefix("child "))
        .filter_map(|pid| pid.trim().parse().ok())
        .collect()
}

#[test]
fn watch_run_stops_the_running_app_tree_before_each_relaunch() {
    let mut probe = Probe::start(
        "dx-watch-probe-run-",
        &["watch", "run", "//app:bin", "--", "--help"],
        &[("FAKE_BAZEL_SLEEP", "30"), ("FAKE_BAZEL_BUILD_SLEEP", "0")],
    );
    probe.until("the app", |probe| probe.count("child ") == 2);
    for round in 2..=3 {
        fs::write(probe.workspace.join("src/main.rs"), format!("edit {round}")).expect("edit");
        probe.until("the relaunched app", |probe| {
            probe.count("child ") == 2 * round
        });
    }
    let log = probe.log();
    assert_eq!(probe.count("start "), 6, "{log}");
    assert_eq!(
        probe.count("interrupted "),
        2,
        "each old app was stopped:\n{log}"
    );
    assert_eq!(
        probe.count("overlap "),
        0,
        "no app outlives its replacement:\n{log}"
    );
    assert!(!starts_before_previous_end(&log), "no overlap:\n{log}");
    let starts: Vec<&str> = log
        .lines()
        .filter(|line| line.starts_with("start "))
        .collect();
    let cache = probe.disk_cache_flag();
    for pair in starts.chunks(2) {
        let build = command_args(pair[0], "build");
        assert!(
            has_cache_and_settings(&build, &cache)
                && build.last() == Some(&"//app:bin")
                && !build.contains(&"--")
                && !build.contains(&"--help"),
            "each launch builds first, without app args: {}",
            pair[0]
        );
        let run = command_args(pair[1], "run");
        assert!(
            has_cache_and_settings(&run, &cache)
                && run.ends_with(&["//app:bin", "--", "--help"])
                && run.iter().filter(|arg| **arg == "--").count() == 1
                && run.iter().filter(|arg| **arg == "--help").count() == 1,
            "the -- payload reaches the app unchanged: {}",
            pair[1]
        );
    }
    let pids = descendants(&probe);
    let (current, old) = pids.split_last().expect("children");
    for old in old {
        assert!(!alive(*old), "old descendant {old} survived:\n{log}");
    }
    let current = *current;
    assert!(alive(current));
    let stdout = probe.stdout();
    assert_eq!(stdout.matches("changed ").count(), 2, "{stdout}");
    assert!(stdout.contains("iteration=3"), "{stdout}");
    let status = probe.stop();
    assert!(
        status.code() == Some(130) || status.signal() == Some(libc::SIGINT),
        "{status:?}\n{}",
        probe.stderr()
    );
    assert!(
        !alive(current),
        "last app {current} survived dx exit\nlog:\n{}",
        probe.log()
    );
    std::thread::sleep(Duration::from_millis(400));
    assert_eq!(probe.count("start "), 6, "no launch after the signal");
}

#[test]
fn watch_run_relaunches_a_finished_app_only_after_a_change() {
    let mut probe = Probe::start(
        "dx-watch-probe-run-exit-",
        &["watch", "run", "//app:bin"],
        &[("FAKE_BAZEL_CODES", "0 4")],
    );
    probe.idle(2);
    probe.settle();
    assert_eq!(probe.count("start "), 2, "{}", probe.log());
    fs::write(probe.workspace.join("src/main.rs"), "edit").expect("edit");
    probe.idle(4);
    let log = probe.log();
    let starts: Vec<&str> = log
        .lines()
        .filter(|line| line.starts_with("start "))
        .collect();
    assert_eq!(starts.len(), 4, "{log}");
    assert!(starts[2].contains(" build "), "{log}");
    assert!(starts[3].contains(" run "), "{log}");
    assert_eq!(probe.count("overlap "), 0, "{log}");
    assert_eq!(probe.stop().code(), Some(130));
}
