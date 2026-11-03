use std::path::PathBuf;
use std::process::{Command, Output};

fn path(var: &str) -> PathBuf {
    dx_testing::resolve_runfiles(&std::env::var(var).expect("path env is set"))
}

fn stock(args: &[&str], cwd: &PathBuf, working_dir: Option<&PathBuf>) -> Output {
    let mut command = Command::new(path("STOCK_BIN"));
    command.args(args).current_dir(cwd);
    command.env_remove("BUILD_WORKING_DIRECTORY");
    command.env_remove("STOCK_FILE");
    if let Some(dir) = working_dir {
        command.env("BUILD_WORKING_DIRECTORY", dir);
    }
    command.output().expect("stock runs")
}

fn scratch() -> PathBuf {
    PathBuf::from(std::env::var("TEST_TMPDIR").expect("TEST_TMPDIR is set"))
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).replace("\r\n", "\n")
}

fn stderr(run: &Output) -> String {
    text(&run.stderr)
}

#[test]
fn unknown_command_exits_2() {
    let run = stock(&["sell"], &scratch(), None);
    assert_eq!(run.status.code(), Some(2));
    assert!(stderr(&run).starts_with("usage:"));
}

#[test]
fn unknown_item_exits_4() {
    let run = stock(&["get", "rivet"], &scratch(), None);
    assert_eq!(run.status.code(), Some(4));
    assert_eq!(stderr(&run), "stock: no item rivet\n");
}

#[test]
fn missing_stock_file_exits_3() {
    let run = stock(&["--file", "absent.json", "list"], &scratch(), None);
    assert_eq!(run.status.code(), Some(3));
    assert_eq!(stderr(&run), "stock: cannot read stock file\n");
}

#[test]
fn malformed_stock_file_exits_3() {
    let bad = path("STOCK_WRONG_TYPE");
    let run = stock(
        &["--file", bad.to_str().expect("utf-8 path"), "list"],
        &scratch(),
        None,
    );
    assert_eq!(run.status.code(), Some(3));
    assert_eq!(stderr(&run), "stock: stock item is malformed\n");
}

#[test]
fn packaged_stock_is_found_from_any_directory() {
    let run = stock(&["total"], &scratch(), None);
    assert_eq!(run.status.code(), Some(0), "{}", stderr(&run));
    assert_eq!(text(&run.stdout), "total 6600\n");
}

#[test]
fn relative_file_resolves_against_the_invoking_directory() {
    let json = path("STOCK_JSON");
    let dir = json.parent().expect("parent").to_path_buf();
    let run = stock(&["--file", "stock.json", "total"], &scratch(), Some(&dir));
    assert_eq!(run.status.code(), Some(0), "{}", stderr(&run));
    assert_eq!(text(&run.stdout), "total 6600\n");
}
