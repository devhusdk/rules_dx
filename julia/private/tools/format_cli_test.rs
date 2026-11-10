use dx_testing::resolve_runfiles;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const DIRTY: &str = "function f(a)\n        return a + 1\nend\n";
const CLEAN: &str = "function f(a)\n    return a + 1\nend\n";
const BROKEN: &str = "function f(\n    x = 1 +\n";

fn scratch(name: &str, text: &str) -> PathBuf {
    let dir = PathBuf::from(std::env::var("TEST_TMPDIR").unwrap_or_else(|_| "/tmp".into()))
        .join(format!("julia_format_{}_{name}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap_or_else(|error| panic!("mkdir: {error}"));
    let file = dir.join(format!("{name}.jl"));
    std::fs::write(&file, text).unwrap_or_else(|error| panic!("write: {error}"));
    file
}

fn run(mode: &str, files: &[&Path]) -> Output {
    let bin = resolve_runfiles(&std::env::var("JULIA_FORMAT").expect("JULIA_FORMAT"));
    Command::new(bin)
        .arg(mode)
        .args(files)
        .output()
        .unwrap_or_else(|error| panic!("spawn: {error}"))
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8(bytes.to_vec()).unwrap_or_else(|error| panic!("utf8: {error}"))
}

#[test]
fn clean_file_passes() {
    let file = scratch("clean", CLEAN);
    let out = run("--check", &[&file]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    assert!(out.stdout.is_empty());
}

#[test]
fn dirty_file_is_listed_then_fixed() {
    let file = scratch("dirty", DIRTY);
    let out = run("--check", &[&file]);
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(text(&out.stdout), format!("{}\n", file.display()));
    assert_eq!(std::fs::read_to_string(&file).unwrap(), DIRTY);
    let fixed = run("--apply", &[&file]);
    assert_eq!(fixed.status.code(), Some(0));
    assert_eq!(std::fs::read_to_string(&file).unwrap(), CLEAN);
    assert_eq!(run("--check", &[&file]).status.code(), Some(0));
}

#[test]
fn syntax_error_reports_position_and_is_never_written() {
    let file = scratch("broken", BROKEN);
    for mode in ["--check", "--apply"] {
        let out = run(mode, &[&file]);
        assert_eq!(out.status.code(), Some(1));
        assert!(out.stdout.is_empty());
        let stderr = text(&out.stderr);
        assert!(
            stderr.starts_with(&format!("{}:2:13: error: ", file.display())),
            "{stderr}"
        );
        assert_eq!(std::fs::read_to_string(&file).unwrap(), BROKEN);
    }
}

#[test]
fn missing_file_is_a_tool_failure() {
    let out = run("--check", &[Path::new("/nonexistent/missing.jl")]);
    assert_eq!(out.status.code(), Some(2));
    assert!(out.stdout.is_empty());
}
