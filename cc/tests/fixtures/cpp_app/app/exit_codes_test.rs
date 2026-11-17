use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::Mutex;

static SPAWN: Mutex<()> = Mutex::new(());

fn isolated(binary: PathBuf, scratch: &Path) -> PathBuf {
    let dir = scratch.join("isolated");
    std::fs::create_dir_all(&dir).expect("isolated dir");
    let copy = dir.join(binary.file_name().expect("binary name"));
    std::fs::copy(&binary, &copy).expect("binary copies");
    copy
}

fn pantry(args: &[&str], working_dir: Option<&PathBuf>, runfiles: bool) -> Output {
    let _spawn = SPAWN
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let scratch = dx_test_scratch::scratch("pantry");
    let binary = dx_testing::env_file("PANTRY_BIN");
    let mut command = Command::new(if runfiles {
        binary
    } else {
        isolated(binary, scratch.path())
    });
    command.args(args).current_dir(scratch.path());
    command.env_remove("BUILD_WORKING_DIRECTORY");
    if !runfiles {
        for var in ["RUNFILES_DIR", "RUNFILES_MANIFEST_FILE", "TEST_SRCDIR"] {
            command.env_remove(var);
        }
    }
    if let Some(dir) = working_dir {
        command.env("BUILD_WORKING_DIRECTORY", dir);
    }
    command.output().expect("pantry runs")
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).replace("\r\n", "\n")
}

fn stderr(run: &Output) -> String {
    text(&run.stderr)
}

#[test]
fn malformed_menu_exits_4() {
    let bad = dx_testing::env_file("PANTRY_BAD_MENU");
    let run = pantry(&[bad.to_str().expect("utf-8 path")], None, true);
    assert_eq!(run.status.code(), Some(4));
    assert!(
        stderr(&run).contains("line 2: bad price"),
        "{}",
        stderr(&run)
    );
}

#[test]
fn missing_menu_exits_4() {
    let run = pantry(&["absent.csv"], None, true);
    assert_eq!(run.status.code(), Some(4));
    assert_eq!(stderr(&run), "pantry: cannot open absent.csv\n");
}

#[test]
fn unavailable_runfiles_exit_3() {
    let run = pantry(&[], None, false);
    assert_eq!(run.status.code(), Some(3), "{}", stderr(&run));
}

#[test]
fn packaged_menu_is_found_from_any_directory() {
    let run = pantry(&[], None, true);
    assert_eq!(run.status.code(), Some(0), "{}", stderr(&run));
    assert!(text(&run.stdout).contains("total dessert 790\n"));
}

#[test]
fn relative_menu_resolves_against_the_invoking_directory() {
    let menu = dx_testing::env_file("PANTRY_MENU");
    let dir = menu.parent().expect("parent").to_path_buf();
    let run = pantry(&["menu.csv"], Some(&dir), true);
    assert_eq!(run.status.code(), Some(0), "{}", stderr(&run));
    assert!(text(&run.stdout).contains("total main 3830\n"));
}
