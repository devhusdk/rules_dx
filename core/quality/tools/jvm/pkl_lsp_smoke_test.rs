use dx_testing::resolve_runfiles;
use std::path::PathBuf;
use std::process::{Command, Output};

fn run(fixture: &str) -> Output {
    let tmp = PathBuf::from(std::env::var("TEST_TMPDIR").unwrap_or_else(|_| "/tmp".into()))
        .join(format!("home_{fixture}"));
    std::fs::create_dir_all(&tmp).unwrap_or_else(|error| panic!("mkdir: {error}"));
    let var =
        |name: &str| resolve_runfiles(&std::env::var(name).unwrap_or_else(|_| panic!("{name}")));
    let file = resolve_runfiles(&format!("_main/core/quality/tools/jvm/testdata/{fixture}"));
    Command::new(var("LSP_CHECK"))
        .args(["--language-id", "pkl", "--timeout-ms", "30000", "--"])
        .arg(var("PKL_LSP_JAVA"))
        .arg(format!("-Duser.home={}", tmp.display()))
        .arg("-jar")
        .arg(var("PKL_LSP_JAR"))
        .arg("--")
        .arg(file)
        .output()
        .unwrap_or_else(|error| panic!("spawn: {error}"))
}

#[test]
fn clean_file_has_no_diagnostics() {
    let out = run("clean.pkl");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{stdout}{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(stdout.trim(), "");
}

#[test]
fn unused_local_is_reported_on_its_line() {
    let out = run("unused.pkl");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_eq!(
        out.status.code(),
        Some(1),
        "{stdout}{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(stdout.contains("\"line\":2"), "{stdout}");
}
