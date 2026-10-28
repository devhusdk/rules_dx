use dx_testing::resolve_runfiles;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const DIRTY: &str = "void main(){print(1);}\n";
const CLEAN: &str = "void main() {\n  print(1);\n}\n";
const BROKEN: &str = "void main( {\n";

fn scratch(name: &str, text: &str) -> PathBuf {
    let dir = PathBuf::from(std::env::var("TEST_TMPDIR").unwrap_or_else(|_| "/tmp".into()))
        .join(format!("dart_format_{}_{name}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap_or_else(|error| panic!("mkdir: {error}"));
    let file = dir.join(format!("{name}.dart"));
    std::fs::write(&file, text).unwrap_or_else(|error| panic!("write: {error}"));
    file
}

fn run(args: &[&str], file: &Path) -> Output {
    let bin = resolve_runfiles(&std::env::var("DART").expect("DART"));
    let home = file.parent().unwrap_or(file).join("home");
    std::fs::create_dir_all(&home).unwrap_or_else(|error| panic!("mkdir: {error}"));
    Command::new(bin)
        .env_clear()
        .env("HOME", &home)
        .env("DART_SUPPRESS_ANALYTICS", "true")
        .arg("--suppress-analytics")
        .arg("format")
        .arg("--language-version=latest")
        .args(args)
        .arg(file)
        .output()
        .unwrap_or_else(|error| panic!("spawn: {error}"))
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8(bytes.to_vec()).unwrap_or_else(|error| panic!("utf8: {error}"))
}

const CHECK: [&str; 2] = ["--output=none", "--set-exit-if-changed"];

#[test]
fn clean_file_passes() {
    let file = scratch("clean", CLEAN);
    let out = run(&CHECK, &file);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
}

#[test]
fn dirty_file_is_listed_then_fixed() {
    let file = scratch("dirty", DIRTY);
    let out = run(&CHECK, &file);
    assert_eq!(out.status.code(), Some(1), "{}", text(&out.stderr));
    assert!(text(&out.stdout).contains(&format!("Changed {}", file.display())));
    let fixed = run(&[], &file);
    assert_eq!(fixed.status.code(), Some(0), "{}", text(&fixed.stderr));
    assert_eq!(std::fs::read_to_string(&file).expect("read"), CLEAN);
}

#[test]
fn syntax_error_exits_65_with_a_location() {
    let file = scratch("broken", BROKEN);
    let out = run(&CHECK, &file);
    assert_eq!(out.status.code(), Some(65));
    assert!(text(&out.stderr).contains(&format!("of {}:", file.display())));
}
