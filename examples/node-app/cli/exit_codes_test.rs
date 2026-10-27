use std::path::PathBuf;
use std::process::{Command, Output};

fn path(var: &str) -> PathBuf {
    dx_testing::resolve_runfiles(&std::env::var(var).expect("path env is set"))
}

fn registry_at(bin: &PathBuf, args: &[&str], working_dir: Option<&PathBuf>) -> Output {
    let mut command = Command::new(bin);
    command.args(args);
    command.env_remove("BUILD_WORKING_DIRECTORY");
    command.env_remove("REGISTRY_FILE");
    if !cfg!(windows) {
        for var in [
            "TEST_SRCDIR",
            "RUNFILES_DIR",
            "RUNFILES_MANIFEST_FILE",
            "RUNFILES_MANIFEST_ONLY",
            "RUNFILES_REPO_MAPPING",
            "JS_BINARY__RUNFILES",
        ] {
            command.env_remove(var);
        }
    }
    if let Some(dir) = working_dir {
        command.env("BUILD_WORKING_DIRECTORY", dir);
    }
    command.output().expect("registry runs")
}

fn registry(args: &[&str], working_dir: Option<&PathBuf>) -> Output {
    registry_at(&path("REGISTRY_BIN"), args, working_dir)
}

fn stdout(run: &Output) -> String {
    String::from_utf8_lossy(&run.stdout).replace("\r\n", "\n")
}

fn stderr(run: &Output) -> String {
    String::from_utf8_lossy(&run.stderr).replace("\r\n", "\n")
}

fn scratch(name: &str) -> PathBuf {
    let dir = PathBuf::from(std::env::var("TEST_TMPDIR").expect("TEST_TMPDIR is set")).join(name);
    std::fs::create_dir_all(&dir).expect("create scratch directory");
    dir
}

#[test]
fn unknown_command_exits_2() {
    let run = registry(&["sell"], None);
    assert_eq!(run.status.code(), Some(2));
    assert!(stderr(&run).starts_with("usage: registry"));
    assert_eq!(stdout(&run), "");
}

#[test]
fn bad_bump_and_bad_range_exit_2() {
    assert_eq!(
        registry(&["next", "core", "huge"], None).status.code(),
        Some(2)
    );
    let run = registry(&["satisfy", "core", "not a range"], None);
    assert_eq!(run.status.code(), Some(2));
    assert_eq!(stderr(&run), "registry: not a range is not a valid range\n");
}

#[test]
fn unknown_package_and_no_match_exit_4() {
    let run = registry(&["latest", "absent"], None);
    assert_eq!(run.status.code(), Some(4));
    assert_eq!(stderr(&run), "registry: no package absent\n");
    let run = registry(&["satisfy", "core", ">=9.0.0"], None);
    assert_eq!(run.status.code(), Some(4));
    assert_eq!(
        stderr(&run),
        "registry: no version of core satisfies >=9.0.0\n"
    );
    let run = registry(&["next", "edge", "patch"], None);
    assert_eq!(run.status.code(), Some(4));
    assert_eq!(
        stderr(&run),
        "registry: package edge has no stable release\n"
    );
}

#[test]
fn missing_file_exits_3() {
    let run = registry(&["--file", "absent.json", "list"], None);
    assert_eq!(run.status.code(), Some(3));
    assert_eq!(stderr(&run), "registry: cannot read registry file\n");
}

#[test]
fn malformed_files_exit_3_with_their_reason() {
    let cases = [
        (
            "REGISTRY_BAD_ENTRY",
            "each package needs a name and a versions array",
        ),
        (
            "REGISTRY_BAD_VERSION",
            "package core has an invalid version 1.0",
        ),
        ("REGISTRY_DUPLICATE", "duplicate package core"),
        ("REGISTRY_NOT_ARRAY", "registry file must hold an array"),
        ("REGISTRY_NOT_JSON", "registry file is not valid JSON"),
    ];
    for (var, reason) in cases {
        let file = path(var);
        let run = registry(
            &["--file", file.to_str().expect("utf-8 path"), "list"],
            None,
        );
        assert_eq!(run.status.code(), Some(3), "{var}");
        assert_eq!(stderr(&run), format!("registry: {reason}\n"), "{var}");
    }
}

#[test]
fn packaged_registry_is_found_from_any_directory() {
    let dir = scratch("elsewhere");
    let run = registry(&["list"], Some(&dir));
    assert_eq!(run.status.code(), Some(0), "{}", stderr(&run));
    assert_eq!(stdout(&run), "core 1.1.3\nnet 2.1.1\nedge none\n");
    let run = registry(&["next", "net", "major"], Some(&dir));
    assert_eq!(stdout(&run), "3.0.0\n");
    let run = registry(&["satisfy", "net", "^2.0.0"], Some(&dir));
    assert_eq!(stdout(&run), "2.1.1\n");
    let run = registry(&["schema"], Some(&dir));
    assert_eq!(stdout(&run), "schema 1\n");
}

#[test]
fn relative_file_resolves_against_the_invoking_directory() {
    let json = path("REGISTRY_JSON");
    let dir = json.parent().expect("parent").to_path_buf();
    let run = registry(&["--file", "packages.json", "latest", "core"], Some(&dir));
    assert_eq!(run.status.code(), Some(0), "{}", stderr(&run));
    assert_eq!(stdout(&run), "1.1.3\n");
}

#[test]
fn environment_file_is_used_when_no_flag_is_given() {
    let dir = scratch("env");
    let file = dir.join("mine.json");
    std::fs::write(&file, r#"[{"name":"solo","versions":["4.5.6"]}]"#).expect("write file");
    let mut command = Command::new(path("REGISTRY_BIN"));
    command.arg("list").env("REGISTRY_FILE", &file);
    let run = command.output().expect("registry runs");
    assert_eq!(stdout(&run), "solo 4.5.6\n");
}

#[test]
fn where_reports_the_staged_entry_and_the_pinned_dependency() {
    let dir = scratch("where");
    let run = registry(&["where", "--flag", "two words"], Some(&dir));
    assert_eq!(run.status.code(), Some(0), "{}", stderr(&run));
    let text = stdout(&run).replace('\\', "/");
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 5, "{text}");
    let entry = lines[0].strip_prefix("entry ").expect("entry line");
    assert!(entry.ends_with("/examples/node-app/cli/main.js"), "{entry}");
    assert!(std::path::Path::new(entry).is_file(), "{entry}");
    let data = lines[1].strip_prefix("data ").expect("data line");
    assert!(
        data.ends_with("/examples/node-app/registry/packages.json"),
        "{data}"
    );
    assert!(std::path::Path::new(data).is_file(), "{data}");
    let semver = lines[2].strip_prefix("semver ").expect("semver line");
    assert!(semver.ends_with(" 7.6.3"), "{semver}");
    assert!(
        semver.contains("/semver@7.6.3/node_modules/semver "),
        "{semver}"
    );
    assert_eq!(lines[3], "same true");
    assert_eq!(lines[4], r#"argv ["where","--flag","two words"]"#);
}

#[cfg(unix)]
#[test]
fn staged_symlink_runs_the_same_entry() {
    let dir = scratch("staged");
    let link = dir.join("registry-link");
    let _ = std::fs::remove_file(&link);
    std::os::unix::fs::symlink(path("REGISTRY_BIN").canonicalize().expect("bin"), &link)
        .expect("symlink");
    let run = registry_at(&link, &["where"], Some(&dir));
    assert_eq!(run.status.code(), Some(0), "{}", stderr(&run));
    assert!(
        stdout(&run).contains("/examples/node-app/cli/main.js\n"),
        "{}",
        stdout(&run)
    );
}
