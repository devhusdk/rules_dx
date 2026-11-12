use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn stock(args: &[&str], cwd: &Path, working_dir: Option<&PathBuf>) -> Output {
    let mut command = Command::new(dx_testing::env_file("STOCK_BIN"));
    command.args(args).current_dir(cwd);
    command.env_remove("BUILD_WORKING_DIRECTORY");
    command.env_remove("STOCK_FILE");
    if let Some(dir) = working_dir {
        command.env("BUILD_WORKING_DIRECTORY", dir);
    }
    command.output().expect("stock runs")
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).replace("\r\n", "\n")
}

fn stderr(run: &Output) -> String {
    text(&run.stderr)
}

#[test]
fn unknown_command_exits_2() {
    let scratch = dx_test_scratch::scratch("c-app-exit");
    let run = stock(&["sell"], scratch.path(), None);
    assert_eq!(run.status.code(), Some(2));
    assert!(stderr(&run).starts_with("usage:"));
}

#[test]
fn unknown_item_exits_4() {
    let scratch = dx_test_scratch::scratch("c-app-exit");
    let run = stock(&["get", "rivet"], scratch.path(), None);
    assert_eq!(run.status.code(), Some(4));
    assert_eq!(stderr(&run), "stock: no item rivet\n");
}

#[test]
fn missing_stock_file_exits_3() {
    let scratch = dx_test_scratch::scratch("c-app-exit");
    let run = stock(&["--file", "absent.json", "list"], scratch.path(), None);
    assert_eq!(run.status.code(), Some(3));
    assert_eq!(stderr(&run), "stock: cannot read stock file\n");
}

#[test]
fn malformed_stock_file_exits_3() {
    let scratch = dx_test_scratch::scratch("c-app-exit");
    let bad = dx_testing::env_file("STOCK_WRONG_TYPE");
    let run = stock(
        &["--file", bad.to_str().expect("utf-8 path"), "list"],
        scratch.path(),
        None,
    );
    assert_eq!(run.status.code(), Some(3));
    assert_eq!(stderr(&run), "stock: stock item is malformed\n");
}

#[test]
fn packaged_stock_is_found_from_any_directory() {
    let scratch = dx_test_scratch::scratch("c-app-exit");
    let run = stock(&["total"], scratch.path(), None);
    assert_eq!(run.status.code(), Some(0), "{}", stderr(&run));
    assert_eq!(text(&run.stdout), "total 6600\n");
}

#[test]
fn relative_file_resolves_against_the_invoking_directory() {
    let scratch = dx_test_scratch::scratch("c-app-exit");
    let json = dx_testing::env_file("STOCK_JSON");
    let dir = json.parent().expect("parent").to_path_buf();
    let run = stock(
        &["--file", "stock.json", "total"],
        scratch.path(),
        Some(&dir),
    );
    assert_eq!(run.status.code(), Some(0), "{}", stderr(&run));
    assert_eq!(text(&run.stdout), "total 6600\n");
}
