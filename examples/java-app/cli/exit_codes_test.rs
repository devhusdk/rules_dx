use std::path::PathBuf;
use std::process::{Command, Output};

fn path(var: &str) -> PathBuf {
    dx_testing::resolve_runfiles(&std::env::var(var).expect("path env is set"))
}

fn invoice(args: &[&str], working_dir: Option<&PathBuf>) -> Output {
    let mut command = Command::new(path("INVOICE_BIN"));
    command.args(args);
    command.env_remove("BUILD_WORKING_DIRECTORY");
    command.env_remove("INVOICE_FILE");
    if let Some(dir) = working_dir {
        command.env("BUILD_WORKING_DIRECTORY", dir);
    }
    command.output().expect("invoice runs")
}

fn stdout(run: &Output) -> String {
    String::from_utf8_lossy(&run.stdout).into_owned()
}

fn stderr(run: &Output) -> String {
    String::from_utf8_lossy(&run.stderr).into_owned()
}

#[test]
fn unknown_command_exits_2() {
    let run = invoice(&["sell"], None);
    assert_eq!(run.status.code(), Some(2));
    assert!(stderr(&run).starts_with("usage:"));
}

#[test]
fn unknown_invoice_exits_4() {
    let run = invoice(&["show", "INV-9"], None);
    assert_eq!(run.status.code(), Some(4));
    assert_eq!(stderr(&run), "invoice: no invoice INV-9\n");
}

#[test]
fn missing_file_exits_3() {
    let run = invoice(&["--file", "absent.json", "list"], None);
    assert_eq!(run.status.code(), Some(3));
    assert_eq!(stderr(&run), "invoice: cannot read invoice file\n");
}

#[test]
fn malformed_files_exit_3_with_their_reason() {
    let cases = [
        (
            "INVOICE_NEGATIVE",
            "quantity must be a whole number from 0 to 1000000",
        ),
        (
            "INVOICE_FRACTIONAL",
            "quantity must be a whole number from 0 to 1000000",
        ),
        ("INVOICE_WRONG_TYPE", "quantity must be a number"),
        ("INVOICE_NOT_ARRAY", "invoice file must hold an array"),
        ("INVOICE_NOT_JSON", "invoice file is not valid JSON"),
    ];
    for (var, reason) in cases {
        let file = path(var);
        let run = invoice(
            &["--file", file.to_str().expect("utf-8 path"), "list"],
            None,
        );
        assert_eq!(run.status.code(), Some(3), "{var}");
        assert_eq!(stderr(&run), format!("invoice: {reason}\n"), "{var}");
    }
}

#[test]
fn bundled_invoices_are_found_from_any_directory() {
    let run = invoice(&["total"], None);
    assert_eq!(run.status.code(), Some(0), "{}", stderr(&run));
    assert_eq!(stdout(&run), "total 68.00\n");
}

#[test]
fn relative_file_resolves_against_the_invoking_directory() {
    let json = path("INVOICE_JSON");
    let dir = json.parent().expect("parent").to_path_buf();
    let run = invoice(&["--file", "invoices.json", "customers"], Some(&dir));
    assert_eq!(run.status.code(), Some(0), "{}", stderr(&run));
    assert_eq!(stdout(&run), "acme 57.00\nglobex 11.00\n");
}
