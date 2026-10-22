use std::path::PathBuf;
use std::process::{Command, Output};

fn path(var: &str) -> PathBuf {
    dx_testing::resolve_runfiles(&std::env::var(var).expect("path env is set"))
}

fn grades(args: &[&str], working_dir: Option<&PathBuf>) -> Output {
    let mut command = Command::new(path("GRADES_BIN"));
    command.args(args);
    command.env_remove("BUILD_WORKING_DIRECTORY");
    command.env_remove("GRADES_FILE");
    if let Some(dir) = working_dir {
        command.env("BUILD_WORKING_DIRECTORY", dir);
    }
    command.output().expect("grades runs")
}

fn stdout(run: &Output) -> String {
    String::from_utf8_lossy(&run.stdout).into_owned()
}

fn stderr(run: &Output) -> String {
    String::from_utf8_lossy(&run.stderr).into_owned()
}

#[test]
fn unknown_command_exits_2() {
    let run = grades(&["sell"], None);
    assert_eq!(run.status.code(), Some(2));
    assert!(stderr(&run).starts_with("usage:"));
}

#[test]
fn unknown_student_exits_4() {
    let run = grades(&["show", "zed"], None);
    assert_eq!(run.status.code(), Some(4));
    assert_eq!(stderr(&run), "grades: no student zed\n");
}

#[test]
fn missing_file_exits_3() {
    let run = grades(&["--file", "absent.json", "list"], None);
    assert_eq!(run.status.code(), Some(3));
    assert_eq!(stderr(&run), "grades: cannot read grade file\n");
}

#[test]
fn malformed_files_exit_3_with_their_reason() {
    let range = "score must be a whole number from 0 to 100";
    let cases = [
        ("GRADES_ABOVE_MAX", range),
        ("GRADES_NEGATIVE", range),
        ("GRADES_WRONG_TYPE", "score must be a number"),
        ("GRADES_EMPTY", "scores must not be empty"),
        ("GRADES_NOT_ARRAY", "grade file must hold an array"),
        ("GRADES_NOT_JSON", "grade file is not valid JSON"),
    ];
    for (var, reason) in cases {
        let file = path(var);
        let run = grades(
            &["--file", file.to_str().expect("utf-8 path"), "list"],
            None,
        );
        assert_eq!(run.status.code(), Some(3), "{var}");
        assert_eq!(stderr(&run), format!("grades: {reason}\n"), "{var}");
    }
}

#[test]
fn bundled_grades_are_found_from_any_directory() {
    let run = grades(&["ranks"], None);
    assert_eq!(run.status.code(), Some(0), "{}", stderr(&run));
    assert_eq!(stdout(&run), "1 cy 96.7 A\n2 ada 91.7 A\n3 bo 72.3 C\n");
}

#[test]
fn relative_file_resolves_against_the_invoking_directory() {
    let json = path("GRADES_JSON");
    let dir = json.parent().expect("parent").to_path_buf();
    let run = grades(&["--file", "grades.json", "letters"], Some(&dir));
    assert_eq!(run.status.code(), Some(0), "{}", stderr(&run));
    assert_eq!(stdout(&run), "A cy ada\nC bo\n");
}
