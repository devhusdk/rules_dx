//! Runs wrapper/bazel.ps1 the way Bazelisk does, with a fake Bazel and a fake PATH.
//! SystemRoot stays real: PowerShell does not start without it.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

struct Host {
    _dir: dx_test_scratch::TempDir,
    root: PathBuf,
}

impl Host {
    fn new(files: &[&str]) -> Host {
        let dir = dx_test_scratch::scratch("bazel-wrapper");
        // The wrapper prints paths with backslashes; TEST_TMPDIR may use forward slashes.
        let root = PathBuf::from(dir.path().to_string_lossy().replace('/', "\\"));
        for file in files {
            let path = root.join(file);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, "").unwrap();
        }
        Host { _dir: dir, root }
    }

    fn path(&self, rel: &str) -> PathBuf {
        self.root.join(rel)
    }

    fn command(&self, path: &[&str], args: &[&str]) -> Command {
        let mut cmd = Command::new(system32().join(r"WindowsPowerShell\v1.0\powershell.exe"));
        cmd.args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"])
            .arg(dx_testing::resolve_runfiles(
                &std::env::var("DX_BAZEL_WRAPPER").unwrap(),
            ))
            .args(args)
            .env(
                "PATH",
                std::env::join_paths(
                    path.iter()
                        .map(|dir| self.path(dir))
                        .chain(std::iter::once(system32())),
                )
                .unwrap(),
            )
            .env(
                "BAZEL_REAL",
                dx_testing::resolve_runfiles(&std::env::var("DX_FAKE_BAZEL").unwrap()),
            )
            .env_remove("BAZEL_SH")
            .env_remove("FAKE_BAZEL_EXIT");
        cmd
    }
}

fn system32() -> PathBuf {
    Path::new(&std::env::var("SystemRoot").unwrap()).join("System32")
}

fn run(cmd: &mut Command) -> (Output, Vec<String>) {
    let output = cmd.output().unwrap();
    let lines = String::from_utf8(output.stdout.clone())
        .unwrap()
        .lines()
        .map(str::to_owned)
        .collect();
    (output, lines)
}

const GIT: [&str; 3] = [
    r"Other\bash.exe",
    r"Git\cmd\git.exe",
    r"Git\usr\bin\bash.exe",
];

#[test]
fn an_unset_bazel_sh_becomes_the_git_bash_next_to_git() {
    let host = Host::new(&GIT);
    let (output, lines) = run(&mut host.command(&[r"Other", r"Git\cmd"], &["version"]));
    assert!(output.status.success(), "{output:?}");
    let git_bash = host.path(r"Git\usr\bin\bash.exe");
    assert_eq!(
        lines,
        [
            format!("BAZEL_SH={}", git_bash.display()),
            "arg=version".to_owned()
        ]
    );
}

#[test]
fn a_bazel_sh_the_user_set_is_kept() {
    let host = Host::new(&GIT);
    let mut cmd = host.command(&[r"Other", r"Git\cmd"], &[]);
    let (_, lines) = run(cmd.env("BAZEL_SH", r"D:\tools\bash.exe"));
    assert_eq!(lines, [r"BAZEL_SH=D:\tools\bash.exe"]);
}

#[test]
fn bazel_sh_stays_unset_when_git_bash_is_already_first_on_path() {
    let host = Host::new(&GIT);
    let (_, lines) = run(&mut host.command(&[r"Git\usr\bin", r"Other", r"Git\cmd"], &[]));
    assert_eq!(lines, ["BAZEL_SH="]);
}

#[test]
fn without_git_bash_only_the_wsl_launcher_warns_and_bazel_still_runs() {
    let host = Host::new(&[r"Other\bash.exe"]);
    let (output, lines) = run(&mut host.command(&[], &["info"]));
    assert!(output.status.success(), "{output:?}");
    assert_eq!(lines, ["BAZEL_SH=", "arg=info"]);
    let wsl = system32().join("bash.exe").exists();
    let warned = String::from_utf8_lossy(&output.stderr).contains("set BAZEL_SH");
    assert_eq!(warned, wsl, "{output:?}");

    let (output, _) = run(&mut host.command(&["Other"], &["info"]));
    assert!(output.status.success(), "{output:?}");
    let warned = String::from_utf8_lossy(&output.stderr).contains("set BAZEL_SH");
    assert!(!warned, "{output:?}");
}

#[test]
fn arguments_reach_bazel_unchanged() {
    let host = Host::new(&GIT);
    let args = [
        "test",
        "--disk_cache=",
        "--test_env=X=a b",
        r#"--define=q="y z""#,
        "",
        r"C:\dir with space\",
        "-c",
        "opt",
        "--",
        "%PATH%",
    ];
    let (output, lines) = run(&mut host.command(&[r"Git\cmd"], &args));
    assert!(output.status.success(), "{output:?}");
    let passed: Vec<&str> = lines[1..]
        .iter()
        .map(|line| line.strip_prefix("arg=").unwrap())
        .collect();
    assert_eq!(passed, args);
}

#[test]
fn the_bazel_exit_code_passes_through() {
    let host = Host::new(&GIT);
    let mut cmd = host.command(&[r"Git\cmd"], &["test"]);
    let (output, _) = run(cmd.env("FAKE_BAZEL_EXIT", "3"));
    assert_eq!(output.status.code(), Some(3));
}
