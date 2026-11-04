use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

const MODULE: &str = "module(name = \"sample\", version = \"0.0.0\")\nbazel_dep(name = \"rules_dx\", version = \"0.0.0\")\n";

struct Watch {
    _scratch: dx_test_scratch::TempDir,
    workspace: PathBuf,
    state: PathBuf,
    child: Child,
}

fn dx_binary() -> PathBuf {
    dx_testing::runfiles_root().join("cli/cli/dx")
}

impl Watch {
    fn start() -> Self {
        Self::start_with(None)
    }

    fn start_with(tree: Option<&str>) -> Self {
        let scratch = dx_test_scratch::scratch("dx-watch-relaunch-");
        let base = scratch.path().to_path_buf();
        let workspace = base.join("ws");
        let bin = base.join("bin");
        let state = base.join("state");
        let tmp = base.join("tmp");
        for dir in [&workspace, &bin, &state, &tmp, &workspace.join(".dx")] {
            fs::create_dir_all(dir).expect("probe dir");
        }
        fs::create_dir_all(workspace.join("src")).expect("src");
        fs::create_dir_all(workspace.join("app")).expect("app");
        fs::write(workspace.join("MODULE.bazel"), MODULE).expect("module");
        fs::write(workspace.join(".dx/version"), "0.0.0\n").expect("pin");
        fs::write(workspace.join("BUILD.bazel"), "").expect("build file");
        fs::write(workspace.join("app/BUILD.bazel"), "").expect("app build file");
        fs::write(workspace.join("src/main.rs"), "fn main() {}\n").expect("source");
        fs::copy(
            dx_testing::process_probe(),
            bin.join(format!("bazel{}", std::env::consts::EXE_SUFFIX)),
        )
        .expect("fake bazel");
        let mut command = Command::new(dx_binary());
        command
            .args(["watch", "run", "//app:bin"])
            .current_dir(&workspace)
            .env_clear()
            .env("PATH", path_with(&bin))
            .env("HOME", &base)
            .env("USERPROFILE", &base)
            .env("LOCALAPPDATA", &base)
            .env("APPDATA", &base)
            .env("TMPDIR", &tmp)
            .env("TEMP", &tmp)
            .env("TMP", &tmp)
            .env("PROCESS_PROBE_BAZEL_DIR", &state)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(fs::File::create(base.join("stderr")).expect("stderr file"));
        if let Some(app_args) = tree {
            command
                .env("PROCESS_PROBE_APP_ARGS", app_args)
                .env("PROCESS_PROBE_APP_DESCENDANT", "1");
        }
        if let Some(root) = std::env::var_os("SystemRoot") {
            command.env("SystemRoot", root);
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            command.creation_flags(CREATE_NO_WINDOW);
        }
        let child = command.spawn().expect("dx starts");
        Watch {
            _scratch: scratch,
            workspace,
            state,
            child,
        }
    }

    fn until(&mut self, what: &str, done: impl Fn(&Path) -> bool) {
        let deadline = Instant::now() + Duration::from_secs(30);
        while !done(&self.state) {
            if let Ok(Some(status)) = self.child.try_wait() {
                panic!("dx exited ({status:?}) before {what}");
            }
            assert!(
                Instant::now() < deadline,
                "timed out waiting for {what}\n{}\nstate: {:?}",
                self.stderr(),
                fs::read_dir(&self.state).map(|dir| dir
                    .filter_map(Result::ok)
                    .map(|entry| entry.file_name())
                    .collect::<Vec<_>>())
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    fn stderr(&self) -> String {
        fs::read_to_string(self.state.with_file_name("stderr")).unwrap_or_default()
    }

    fn launch(&self, n: usize) -> String {
        fs::read_to_string(self.state.join(format!("start-{n}"))).unwrap_or_default()
    }
}

impl Drop for Watch {
    fn drop(&mut self) {
        let _ = fs::write(self.state.join("stop"), "");
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn path_with(bin: &Path) -> std::ffi::OsString {
    let mut dirs = vec![bin.to_path_buf()];
    if cfg!(unix) {
        dirs.extend(["/usr/bin", "/bin"].map(PathBuf::from));
    }
    std::env::join_paths(dirs).expect("path")
}

#[test]
fn watch_run_builds_then_runs_the_app_again_after_each_change() {
    let mut watch = Watch::start();
    watch.until("the first app", |state| state.join("alive-2").exists());
    for round in 1..=2 {
        let next = 2 * round + 2;
        fs::write(watch.workspace.join("src/main.rs"), format!("edit {round}")).expect("edit");
        watch.until("the relaunched app", |state| {
            state.join(format!("alive-{next}")).exists()
        });
    }
    for (n, verb) in [
        (1, "build"),
        (2, "run"),
        (3, "build"),
        (4, "run"),
        (5, "build"),
        (6, "run"),
    ] {
        let launch = watch.launch(n);
        let words: Vec<&str> = launch.split_whitespace().collect();
        assert!(
            words.contains(&verb) && words.last() == Some(&"//app:bin"),
            "launch {n} should be {verb}: {launch}"
        );
    }
    for old in [2, 4] {
        dx_testing::assert_alive_lock_released(&watch.state.join(format!("alive-{old}")));
    }
    assert!(dx_testing::alive_lock_held(&watch.state.join("alive-6")));
}

#[cfg(windows)]
fn ctrl_c_stops_the_watch(app_args: &str) {
    let mut watch = Watch::start_with(Some(app_args));
    let child_lock = watch.state.join("child-2");
    watch.until("the first app", |state| {
        state.join("alive-2").exists() && state.join("child-2").exists()
    });
    let sent = Command::new(dx_testing::process_probe())
        .arg(format!("--ctrl-c-console={}", watch.child.id()))
        .status()
        .expect("helper runs");
    assert!(sent.success(), "the Ctrl-C helper failed: {sent:?}");
    let deadline = Instant::now() + Duration::from_secs(30);
    let status = loop {
        if let Some(status) = watch.child.try_wait().expect("wait for dx") {
            break status;
        }
        assert!(
            Instant::now() < deadline,
            "dx did not stop\n{}",
            watch.stderr()
        );
        std::thread::sleep(Duration::from_millis(20));
    };
    assert_eq!(status.code(), Some(130), "{}", watch.stderr());
    assert!(watch.stderr().contains("stopped"), "{}", watch.stderr());
    dx_testing::assert_alive_lock_released(&watch.state.join("alive-2"));
    dx_testing::assert_alive_lock_released(&child_lock);
    assert!(!watch.state.join("start-3").exists(), "dx relaunched");
}

#[cfg(windows)]
#[test]
fn ctrl_c_stops_the_watch_and_the_app_tree() {
    ctrl_c_stops_the_watch("");
}

#[cfg(windows)]
#[test]
fn ctrl_c_stops_an_app_that_ignores_it() {
    ctrl_c_stops_the_watch("--ignore-console-ctrl");
}
