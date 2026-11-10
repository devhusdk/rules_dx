//! `clang_tidy_run.sh` expands Apple's `__BAZEL_*` placeholders and logs the wrapped tool's output.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use dx_testing::runfiles_root;

struct Harness {
    runner: PathBuf,
    fake: PathBuf,
    tmp: PathBuf,
}

fn harness(name: &str) -> Harness {
    let runner = runfiles_root().join("cc/private/clang_tidy_run.sh");
    let tmp = Path::new(&std::env::var("TEST_TMPDIR").expect("TEST_TMPDIR is set under Bazel"))
        .join(name);
    std::fs::create_dir_all(&tmp).expect("create tmp");
    let fake = tmp.join("fake_tidy");
    std::fs::write(&fake, "#!/usr/bin/env bash\nfor a in \"$@\"; do echo \"ARG:$a\"; done\n")
        .expect("write fake tidy");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755))
            .expect("chmod fake tidy");
    }
    Harness { runner, fake, tmp }
}

impl Harness {
    fn run(&self, cwd: &Path, env: &[(&str, &str)], unset: &[&str], log: &Path, args: &[&str]) -> Output {
        let mut cmd = Command::new("bash");
        cmd.arg(&self.runner).arg(log).arg(&self.fake).args(args).current_dir(cwd);
        for key in ["DEVELOPER_DIR", "SDKROOT"] {
            cmd.env_remove(key);
        }
        for key in unset {
            cmd.env_remove(key);
        }
        cmd.envs(env.iter().copied());
        cmd.output().expect("spawn runner")
    }
}

fn lines(log: &Path) -> Vec<String> {
    std::fs::read_to_string(log).expect("readable log").lines().map(str::to_owned).collect()
}

#[test]
fn placeholders_expand_from_env_and_cwd() {
    let h = harness("expand");
    let log = h.tmp.join("a.log");
    let out = h.run(
        &h.tmp,
        &[("DEVELOPER_DIR", "/dev"), ("SDKROOT", "/sdk")],
        &[],
        &log,
        &["-isysroot", "__BAZEL_XCODE_SDKROOT__", "-F__BAZEL_XCODE_DEVELOPER_DIR__/x", "-I__BAZEL_EXECUTION_ROOT__/inc"],
    );
    assert!(out.status.success());
    let got = lines(&log);
    let cwd = h.tmp.to_string_lossy().into_owned();
    assert_eq!(
        got,
        vec!["ARG:-isysroot", "ARG:/sdk", "ARG:-F/dev/x", &format!("ARG:-I{cwd}/inc")]
    );
}

#[test]
fn ampersands_are_not_special_in_replacements() {
    let h = harness("amp");
    let dir = h.tmp.join("a&b");
    std::fs::create_dir_all(&dir).expect("create dir");
    let log = h.tmp.join("amp.log");
    let out = h.run(
        &dir,
        &[("DEVELOPER_DIR", "/d&v"), ("SDKROOT", "/s&k")],
        &[],
        &log,
        &["__BAZEL_XCODE_SDKROOT__", "-F__BAZEL_XCODE_DEVELOPER_DIR__/x", "-I__BAZEL_EXECUTION_ROOT__/inc"],
    );
    assert!(out.status.success());
    let got = lines(&log);
    assert_eq!(got[0], "ARG:/s&k");
    assert_eq!(got[1], "ARG:-F/d&v/x");
    assert_eq!(got[2], format!("ARG:-I{}/inc", dir.to_string_lossy()));
}

#[test]
fn unset_env_with_xcode_placeholder_fails_without_running_tidy() {
    let h = harness("unset");
    let log = h.tmp.join("b.log");
    let out = h.run(&h.tmp, &[], &[], &log, &["-isysroot", "__BAZEL_XCODE_SDKROOT__"]);
    assert!(!out.status.success());
    assert!(!log.exists(), "tidy must not run with a raw placeholder");
}

#[test]
fn plain_arguments_pass_through() {
    let h = harness("plain");
    let log = h.tmp.join("c.log");
    let out = h.run(&h.tmp, &[], &[], &log, &["-DX=1", "a.c"]);
    assert!(out.status.success());
    assert_eq!(lines(&log), vec!["ARG:-DX=1", "ARG:a.c"]);
}
