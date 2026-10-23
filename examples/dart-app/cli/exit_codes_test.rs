use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn path(var: &str) -> PathBuf {
    dx_testing::resolve_runfiles(&std::env::var(var).expect("path env is set"))
}

fn ledger(args: &[&str], working_dir: Option<&Path>) -> Output {
    let mut command = Command::new(path("LEDGER_BIN"));
    command.args(args);
    command.env_remove("BUILD_WORKING_DIRECTORY");
    command.env_remove("LEDGER_FILE");
    if let Some(dir) = working_dir {
        command.env("BUILD_WORKING_DIRECTORY", dir);
    }
    command.output().expect("ledger runs")
}

fn stdout(run: &Output) -> String {
    String::from_utf8_lossy(&run.stdout).replace("\r\n", "\n")
}

fn stderr(run: &Output) -> String {
    String::from_utf8_lossy(&run.stderr).replace("\r\n", "\n")
}

#[test]
fn packaged_ledger_totals_by_category() {
    let run = ledger(&["totals"], None);
    assert_eq!(run.status.code(), Some(0), "{}", stderr(&run));
    assert_eq!(
        stdout(&run),
        "rent 1200.00\nfood 73.30\nutilities 64.20\ntravel 48.00\n"
    );
}

#[test]
fn version_comes_from_the_build_defines() {
    let run = ledger(&["--version"], None);
    assert_eq!(run.status.code(), Some(0), "{}", stderr(&run));
    assert_eq!(stdout(&run), "ledger 2.4.0\n");
}

#[test]
fn usage_errors_exit_2() {
    for args in [
        &[][..],
        &["sell"][..],
        &["total"][..],
        &["--colour", "list"][..],
    ] {
        let run = ledger(args, None);
        assert_eq!(run.status.code(), Some(2), "{args:?}");
        assert!(stderr(&run).contains("usage: ledger"), "{args:?}");
    }
}

#[test]
fn unknown_category_exits_4() {
    let run = ledger(&["total", "toys"], None);
    assert_eq!(run.status.code(), Some(4));
    assert_eq!(stderr(&run), "ledger: no category toys\n");
}

#[test]
fn missing_file_exits_3() {
    let run = ledger(&["--file", "absent.csv", "list"], None);
    assert_eq!(run.status.code(), Some(3));
    assert_eq!(stderr(&run), "ledger: cannot read ledger file\n");
}

#[test]
fn malformed_files_exit_3_with_their_reason() {
    let cases = [
        ("LEDGER_BAD_DATE", "line 2: date must be YYYY-MM-DD"),
        (
            "LEDGER_BAD_HEADER",
            "line 1: header must be \"date,category,cents\"",
        ),
        ("LEDGER_EMPTY", "ledger has no entries"),
        (
            "LEDGER_NEGATIVE",
            "line 2: cents must be a positive whole number",
        ),
        ("LEDGER_SHORT_LINE", "line 2: want 3 fields, got 2"),
        ("LEDGER_UNKNOWN_CATEGORY", "line 2: unknown category toys"),
    ];
    for (var, reason) in cases {
        let file = path(var);
        let run = ledger(
            &["--file", file.to_str().expect("utf-8 path"), "totals"],
            None,
        );
        assert_eq!(run.status.code(), Some(3), "{var}");
        assert_eq!(stderr(&run), format!("ledger: {reason}\n"), "{var}");
    }
}

#[test]
fn relative_file_resolves_against_the_invoking_directory() {
    let csv = path("LEDGER_CSV");
    let dir = csv.parent().expect("parent");
    let run = ledger(&["--file", "entries.csv", "total", "food"], Some(dir));
    assert_eq!(run.status.code(), Some(0), "{}", stderr(&run));
    assert_eq!(stdout(&run), "food 73.30\n");
}
