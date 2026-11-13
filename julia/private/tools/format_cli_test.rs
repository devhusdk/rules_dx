use dx_testing::env_file;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const DIRTY: &str = "function f(a)\n        return a + 1\nend\n";
const CLEAN: &str = "function f(a)\n    return a + 1\nend\n";
const BROKEN: &str = "function f(\n    x = 1 +\n";

fn source(dir: &Path, name: &str, text: &str) -> PathBuf {
    let file = dir.join(format!("{name}.jl"));
    std::fs::write(&file, text).unwrap_or_else(|error| panic!("write: {error}"));
    file
}

fn run(mode: &str, files: &[&Path]) -> Output {
    let bin = env_file("JULIA_FORMAT");
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
    let scratch = dx_test_scratch::scratch("julia-format");
    let file = source(scratch.path(), "clean", CLEAN);
    let out = run("--check", &[&file]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    assert!(out.stdout.is_empty());
}

#[test]
fn dirty_file_is_listed_then_fixed() {
    let scratch = dx_test_scratch::scratch("julia-format");
    let file = source(scratch.path(), "dirty", DIRTY);
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
    let scratch = dx_test_scratch::scratch("julia-format");
    let file = source(scratch.path(), "broken", BROKEN);
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
