use super::*;

fn run(vars: &[(&str, &str)], files: &[&str]) -> Result<Option<PathBuf>, WslBash> {
    let get = |name: &str| {
        vars.iter()
            .find(|(key, _)| *key == name)
            .map(|(_, value)| OsString::from(value))
    };
    let exists = |path: &Path| files.iter().any(|file| Path::new(file) == path);
    resolve_bazel_sh(&get, &exists)
}

#[cfg(unix)]
const PATH: &str = "/win/System32:/git/cmd";
#[cfg(unix)]
const SYS: &str = "/win";

#[cfg(unix)]
#[test]
fn user_value_is_kept() {
    let vars = [
        ("BAZEL_SH", "/mine/bash.exe"),
        ("PATH", PATH),
        ("SystemRoot", SYS),
    ];
    let files = [
        "/win/System32/bash.exe",
        "/git/cmd/git.exe",
        "/git/usr/bin/bash.exe",
    ];
    assert_eq!(run(&vars, &files), Ok(None));
}

#[cfg(unix)]
#[test]
fn git_bash_wins_over_wsl() {
    let vars = [("PATH", PATH), ("SystemRoot", SYS)];
    let files = [
        "/win/System32/bash.exe",
        "/git/cmd/git.exe",
        "/git/usr/bin/bash.exe",
    ];
    assert_eq!(
        run(&vars, &files),
        Ok(Some(PathBuf::from("/git/usr/bin/bash.exe")))
    );
}

#[cfg(unix)]
#[test]
fn wsl_only_is_an_error() {
    let vars = [("PATH", PATH), ("SystemRoot", SYS)];
    let err = run(&vars, &["/win/System32/bash.exe"]).unwrap_err();
    assert_eq!(err.0, PathBuf::from("/win/System32/bash.exe"));
    assert!(err.to_string().contains("BAZEL_SH"));
}

#[cfg(unix)]
#[test]
fn git_without_bash_and_other_bash_is_left_alone() {
    let vars = [("PATH", PATH), ("SystemRoot", SYS)];
    assert_eq!(run(&vars, &["/git/cmd/git.exe"]), Ok(None));
    let vars = [("PATH", "/msys/bin"), ("SystemRoot", SYS)];
    assert_eq!(run(&vars, &["/msys/bin/bash.exe"]), Ok(None));
}

#[cfg(unix)]
#[test]
fn no_bash_leaves_env_unchanged() {
    let vars = [("PATH", PATH), ("SystemRoot", SYS)];
    assert_eq!(run(&vars, &[]), Ok(None));
}
