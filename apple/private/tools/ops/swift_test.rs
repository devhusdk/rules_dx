//! Tests the swift operation.

mod fake;

use std::path::PathBuf;

use dx_apple_ops::swift::run;
use dx_apple_ops::util::Output;
use dx_apple_ops::xcode::{Tools, XCRUN};
use fake::{Fake, args, fail, ok};

fn tools(fake: &Fake) -> Tools<'_> {
    Tools {
        developer_dir: Some(PathBuf::from("/x/Developer")),
        runner: fake,
    }
}

#[test]
fn swift_runs_through_xcrun_with_the_chosen_xcode() {
    let fake = Fake::new(|_| ok(""));
    let outcome = run(&tools(&fake), &args(&["package", "resolve"]));
    assert_eq!(outcome, Ok(0));
    let calls = fake.calls.borrow();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].program, XCRUN);
    assert_eq!(calls[0].args, args(&["swift", "package", "resolve"]));
    assert_eq!(
        calls[0].env,
        [("DEVELOPER_DIR".to_owned(), "/x/Developer".to_owned())]
    );
}

#[test]
fn swift_passes_the_exit_code_through() {
    let fake = Fake::new(|_| fail(3, ""));
    assert_eq!(run(&tools(&fake), &args(&["build"])), Ok(3));
}

#[test]
fn swift_reports_a_missing_xcrun() {
    let fake = Fake::new(|_| Output {
        code: None,
        stdout: String::new(),
        stderr: String::new(),
    });
    let failure = run(&tools(&fake), &args(&[])).unwrap_err();
    assert_eq!(failure.code, 127);
    assert!(failure.message.starts_with(&format!("cannot start {XCRUN}")));
}
