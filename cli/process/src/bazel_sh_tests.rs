use super::*;

fn run(vars: &[(&str, &str)], files: &[&str]) -> Result<BazelSh, WslBash> {
    run_with_server(vars, files, None)
}

fn run_with_server(
    vars: &[(&str, &str)],
    files: &[&str],
    cmdline: Option<&[u8]>,
) -> Result<BazelSh, WslBash> {
    let get = |name: &str| {
        vars.iter()
            .find(|(key, _)| *key == name)
            .map(|(_, value)| OsString::from(value))
    };
    let exists = |path: &Path| files.iter().any(|file| Path::new(file) == path);
    let read = |path: &Path| {
        (path == Path::new(OUTPUT_BASE).join("server").join("cmdline"))
            .then_some(cmdline)
            .flatten()
            .map(<[u8]>::to_vec)
    };
    resolve_bazel_sh(&get, &exists, &read)
}

fn path(dirs: &[&str]) -> String {
    std::env::join_paths(dirs)
        .expect("joinable")
        .into_string()
        .expect("utf8")
}

const SYS: &str = "/win";
const WIN_DIRS: [&str; 2] = ["/win/System32", "/git/cmd"];
const DETECTED: &str = r"C:\Program Files\Git\usr\bin\bash.exe";
const OUTPUT_BASE: &str = if cfg!(windows) { "C:/ob" } else { "/ob" };
const EXECROOT: &str = if cfg!(windows) {
    "C:/ob/execroot/_main"
} else {
    "/ob/execroot/_main"
};

fn server(root: &str) -> Vec<u8> {
    format!(
        "java\0-Xverify:none\0-Dbazel.windows_unix_root={root}\0--host_jvm_args=-Dbazel.windows_unix_root={root}\0"
    )
    .into_bytes()
}

#[test]
fn user_value_is_kept() {
    let p = path(&WIN_DIRS);
    let vars = [
        ("BAZEL_SH", "/mine/bash.exe"),
        ("PATH", p.as_str()),
        ("SystemRoot", SYS),
    ];
    let files = [
        "/win/System32/bash.exe",
        "/git/cmd/git.exe",
        "/git/usr/bin/bash.exe",
    ];
    assert_eq!(run(&vars, &files), Ok(BazelSh::Keep));
}

#[test]
fn bash_the_running_server_detected_is_unset() {
    let vars = [("BAZEL_SH", DETECTED), ("BUILD_EXECROOT", EXECROOT)];
    let cmdline = server(DETECTED);
    assert_eq!(
        run_with_server(&vars, &[], Some(&cmdline)),
        Ok(BazelSh::Unset)
    );
}

#[test]
fn bash_the_running_server_read_from_bazel_sh_is_kept() {
    let vars = [("BAZEL_SH", DETECTED), ("BUILD_EXECROOT", EXECROOT)];
    let cmdline = server("C:/Program Files/Git/usr/bin/bash.exe");
    assert_eq!(
        run_with_server(&vars, &[], Some(&cmdline)),
        Ok(BazelSh::Keep)
    );
    let forward = [
        ("BAZEL_SH", "C:/Program Files/Git/usr/bin/bash.exe"),
        ("BUILD_EXECROOT", EXECROOT),
    ];
    assert_eq!(
        run_with_server(&forward, &[], Some(&cmdline)),
        Ok(BazelSh::Keep)
    );
}

#[test]
fn bazel_sh_without_a_known_server_is_kept() {
    let cmdline = server(DETECTED);
    let outside_run = [("BAZEL_SH", DETECTED)];
    assert_eq!(
        run_with_server(&outside_run, &[], Some(&cmdline)),
        Ok(BazelSh::Keep)
    );
    let no_server = [("BAZEL_SH", DETECTED), ("BUILD_EXECROOT", EXECROOT)];
    assert_eq!(run_with_server(&no_server, &[], None), Ok(BazelSh::Keep));
    let other_server = [("BAZEL_SH", DETECTED), ("BUILD_EXECROOT", EXECROOT)];
    let without_root = b"java\0-Xverify:none\0".to_vec();
    assert_eq!(
        run_with_server(&other_server, &[], Some(&without_root)),
        Ok(BazelSh::Keep)
    );
}

#[test]
fn git_bash_wins_over_wsl() {
    let p = path(&WIN_DIRS);
    let vars = [("PATH", p.as_str()), ("SystemRoot", SYS)];
    let files = [
        "/win/System32/bash.exe",
        "/git/cmd/git.exe",
        "/git/usr/bin/bash.exe",
    ];
    assert_eq!(
        run(&vars, &files),
        Ok(BazelSh::Set(PathBuf::from("/git/usr/bin/bash.exe")))
    );
}

#[test]
fn git_bash_first_on_path_is_left_to_bazel() {
    let p = path(&["/git/usr/bin", "/win/System32", "/git/cmd"]);
    let vars = [("PATH", p.as_str()), ("SystemRoot", SYS)];
    let files = [
        "/win/System32/bash.exe",
        "/git/cmd/git.exe",
        "/git/usr/bin/bash.exe",
    ];
    assert_eq!(run(&vars, &files), Ok(BazelSh::Keep));
}

#[test]
fn wsl_only_is_an_error() {
    let p = path(&WIN_DIRS);
    let vars = [("PATH", p.as_str()), ("SystemRoot", SYS)];
    let err = run(&vars, &["/win/System32/bash.exe"]).unwrap_err();
    assert_eq!(err.0, PathBuf::from("/win/System32/bash.exe"));
    assert!(err.to_string().contains("BAZEL_SH"));
}

#[test]
fn git_without_bash_and_other_bash_is_left_alone() {
    let p = path(&WIN_DIRS);
    let m = path(&["/msys/bin"]);
    let vars = [("PATH", p.as_str()), ("SystemRoot", SYS)];
    assert_eq!(run(&vars, &["/git/cmd/git.exe"]), Ok(BazelSh::Keep));
    let vars = [("PATH", m.as_str()), ("SystemRoot", SYS)];
    assert_eq!(run(&vars, &["/msys/bin/bash.exe"]), Ok(BazelSh::Keep));
}

#[test]
fn no_bash_leaves_env_unchanged() {
    let p = path(&WIN_DIRS);
    let vars = [("PATH", p.as_str()), ("SystemRoot", SYS)];
    assert_eq!(run(&vars, &[]), Ok(BazelSh::Keep));
}
