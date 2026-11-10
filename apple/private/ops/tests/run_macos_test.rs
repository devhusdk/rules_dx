//! Tests running a built macOS app.

mod fake;

use std::path::PathBuf;

use dx_apple_ops::run_macos::run;
use dx_apple_ops::util::Output;
use dx_apple_ops::xcode::{DITTO, PLUTIL, Tools};
use dx_test_scratch::{scratch, write};
use fake::{Fake, args, env, fail, ok};

fn tools(fake: &Fake) -> Tools<'_> {
    Tools {
        developer_dir: None,
        runner: fake,
    }
}

fn scripted(app: bool) -> Fake {
    Fake::new(move |cmd| match cmd.program.as_str() {
        DITTO => {
            if app {
                let target = PathBuf::from(&cmd.args[3]).join("Ledger.app/Contents");
                std::fs::create_dir_all(&target).unwrap();
            }
            ok("")
        }
        PLUTIL => ok("Ledger\n"),
        _ => ok(""),
    })
}

#[test]
fn the_app_is_unpacked_and_run_with_the_arguments() {
    let root = scratch("macos-run");
    write(root.path(), "Ledger.zip", "PK");
    let zipped = root.path().join("Ledger.zip").to_string_lossy().into_owned();
    let tmp = root.path().join("tmp");
    std::fs::create_dir_all(&tmp).unwrap();
    let tmp_text = tmp.to_string_lossy().into_owned();
    let fake = scripted(true);
    let outcome = run(
        &tools(&fake),
        &args(&[&zipped, "--flag", "two"]),
        &env(&[("TMPDIR", &tmp_text)]),
    );
    assert_eq!(outcome, Ok(0));
    let calls = fake.calls.borrow();
    assert_eq!(calls.len(), 3);
    assert_eq!(calls[0].program, DITTO);
    assert_eq!(&calls[0].args[..3], ["-x", "-k", zipped.as_str()]);
    assert_eq!(calls[1].program, PLUTIL);
    assert!(calls[2].program.ends_with("Ledger.app/Contents/MacOS/Ledger"));
    assert_eq!(calls[2].args, ["--flag", "two"]);
    assert_eq!(std::fs::read_dir(&tmp).unwrap().count(), 0);
}

#[test]
fn the_exit_code_of_the_app_is_kept() {
    let root = scratch("macos-code");
    write(root.path(), "Ledger.zip", "PK");
    let zipped = root.path().join("Ledger.zip").to_string_lossy().into_owned();
    let tmp = root.path().to_string_lossy().into_owned();
    let fake = Fake::new(|cmd| {
        if cmd.program == DITTO {
            std::fs::create_dir_all(PathBuf::from(&cmd.args[3]).join("Ledger.app")).unwrap();
            ok("")
        } else if cmd.program == PLUTIL {
            ok("Ledger")
        } else {
            fail(7, "")
        }
    });
    assert_eq!(
        run(&tools(&fake), &args(&[&zipped]), &env(&[("TMPDIR", &tmp)])),
        Ok(7)
    );
}

#[test]
fn a_missing_archive_or_argument_fails_before_running_anything() {
    let fake = scripted(true);
    let none = run(&tools(&fake), &args(&[]), &env(&[])).unwrap_err();
    assert_eq!(none.code, 2);
    let gone = run(&tools(&fake), &args(&["/nope.zip"]), &env(&[])).unwrap_err();
    assert_eq!(gone.message, "/nope.zip is not in the runfiles");
    assert!(fake.calls.borrow().is_empty());
}

#[test]
fn a_host_without_ditto_fails_with_127() {
    let root = scratch("macos-noditto");
    write(root.path(), "Ledger.zip", "PK");
    let zipped = root.path().join("Ledger.zip").to_string_lossy().into_owned();
    let tmp = root.path().to_string_lossy().into_owned();
    let fake = Fake::new(|_| Output {
        code: None,
        stdout: String::new(),
        stderr: "No such file".to_owned(),
    });
    let failure = run(&tools(&fake), &args(&[&zipped]), &env(&[("TMPDIR", &tmp)])).unwrap_err();
    assert_eq!(failure.code, 127);
    assert!(failure.message.ends_with("running the app needs macOS"));
}

#[test]
fn an_archive_without_an_app_fails() {
    let root = scratch("macos-noapp");
    write(root.path(), "Ledger.zip", "PK");
    let zipped = root.path().join("Ledger.zip").to_string_lossy().into_owned();
    let tmp = root.path().to_string_lossy().into_owned();
    let fake = scripted(false);
    let failure = run(&tools(&fake), &args(&[&zipped]), &env(&[("TMPDIR", &tmp)])).unwrap_err();
    assert_eq!(failure.code, 1);
    assert_eq!(failure.message, "the archive holds no .app");
}

#[test]
fn a_failed_unpack_keeps_the_ditto_exit_code() {
    let root = scratch("macos-badzip");
    write(root.path(), "Ledger.zip", "PK");
    let zipped = root.path().join("Ledger.zip").to_string_lossy().into_owned();
    let tmp = root.path().to_string_lossy().into_owned();
    let fake = Fake::new(|_| fail(9, "corrupt"));
    let failure = run(&tools(&fake), &args(&[&zipped]), &env(&[("TMPDIR", &tmp)])).unwrap_err();
    assert_eq!(failure.code, 9);
}
