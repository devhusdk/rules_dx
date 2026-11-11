//! Tests notarizing and stapling a signed macOS app.

mod fake;

use dx_apple_ops::notarize::run;
use dx_apple_ops::util::{Cmd, Output};
use dx_apple_ops::xcode::{CODESIGN, DITTO, PLUTIL, SPCTL, Tools, XCRUN};
use dx_test_scratch::{scratch, write};
use fake::{Fake, args, env, fail, ok};

const DETAILS: &str = "CodeDirectory v=20500 flags=0x10000(runtime) hashes=1\nSignature size=9000\n";

struct Setup {
    root: dx_test_scratch::TempDir,
    tmp: String,
}

fn setup(prefix: &str) -> Setup {
    let root = scratch(prefix);
    write(root.path(), "dist/Ledger.app/Contents/Info.plist", "plist");
    write(root.path(), "tmp/keep", "");
    Setup { tmp: root.path().join("tmp").to_string_lossy().into_owned(), root }
}

fn notary(status: &'static str, details: &'static str, submit_stderr: &'static str) -> Fake {
    Fake::new(move |cmd: &Cmd| {
        let arg = |i: usize| cmd.args.get(i).cloned().unwrap_or_default();
        match cmd.program.as_str() {
            PLUTIL => match arg(1).as_str() {
                "CFBundleIdentifier" => ok("com.example.ledger\n"),
                "id" if !submit_stderr.is_empty() => fail(1, "none"),
                "id" => ok("sub-1\n"),
                "status" => ok(&format!("{status}\n")),
                _ => fail(1, ""),
            },
            CODESIGN => Output { code: Some(0), stdout: String::new(), stderr: details.to_owned() },
            XCRUN if arg(0) == "notarytool" && arg(1) == "submit" => Output {
                code: Some(if submit_stderr.is_empty() { 0 } else { 1 }),
                stdout: "{}".to_owned(),
                stderr: submit_stderr.to_owned(),
            },
            _ => ok(""),
        }
    })
}

fn call(set: &Setup, fake: &Fake, flags: &[&str]) -> dx_apple_ops::util::Outcome {
    let mut all = vec!["com.example.ledger"];
    all.extend(flags);
    let tools = Tools { developer_dir: None, runner: fake };
    run(&tools, &args(&all), &env(&[("TMPDIR", &set.tmp), ("BUILD_WORKING_DIRECTORY", &set.root.path().to_string_lossy())]))
}

#[test]
fn an_accepted_submission_is_stapled_and_assessed() {
    let set = setup("notarize-ok");
    let fake = notary("Accepted", DETAILS, "");
    let outcome = call(&set, &fake, &["--app", "dist/Ledger.app", "--keychain-profile", "ci"]);
    assert_eq!(outcome, Ok(0));
    let log = fake.log();
    assert!(log.iter().any(|l| l.contains("notarytool submit") && l.contains("--keychain-profile ci --wait --timeout 2h --output-format json")));
    assert!(log.iter().any(|l| l.starts_with("notarytool log sub-1")));
    assert!(log.iter().any(|l| l.starts_with("stapler staple")));
    assert!(log.iter().any(|l| l.starts_with("stapler validate")));
    let calls = fake.calls.borrow();
    assert_eq!(calls.last().map(|c| c.program.as_str()), Some(SPCTL));
    assert!(calls.iter().filter(|c| c.program == DITTO).all(|c| c.args[0] == "-c"));
}

#[test]
fn key_credentials_are_passed_to_notarytool() {
    let set = setup("notarize-key");
    write(set.root.path(), "AuthKey.p8", "key");
    let fake = notary("Accepted", DETAILS, "");
    let outcome = call(&set, &fake, &["--app", "dist/Ledger.app", "--key", "AuthKey.p8", "--key-id", "K1", "--issuer", "I1", "--timeout", "5m"]);
    assert_eq!(outcome, Ok(0));
    assert!(fake.log().iter().any(|l| l.contains("--key-id K1 --issuer I1 --wait --timeout 5m")));
}

#[test]
fn a_rejected_submission_exits_with_one_and_names_the_log() {
    let set = setup("notarize-rejected");
    let fake = notary("Invalid", DETAILS, "");
    let failure = call(&set, &fake, &["--app", "dist/Ledger.app", "--keychain-profile", "ci"]).unwrap_err();
    assert_eq!(failure.code, 1);
    assert!(failure.message.starts_with("submission sub-1 of Ledger.app is Invalid; see the notary log"));
    assert!(!fake.log().iter().any(|l| l.starts_with("stapler")));
}

#[test]
fn a_submission_still_in_progress_exits_with_three() {
    let set = setup("notarize-progress");
    let failure = call(&set, &notary("In Progress", DETAILS, ""), &["--app", "dist/Ledger.app", "--keychain-profile", "ci"]).unwrap_err();
    assert_eq!(failure.code, 3);
}

#[test]
fn an_unreachable_notary_service_exits_with_three() {
    let set = setup("notarize-offline");
    let fake = notary("", DETAILS, "NSURLErrorDomain code=-1009");
    let failure = call(&set, &fake, &["--app", "dist/Ledger.app", "--keychain-profile", "ci"]).unwrap_err();
    assert_eq!(failure.code, 3);
}

#[test]
fn an_app_without_the_hardened_runtime_is_refused() {
    let set = setup("notarize-plain");
    let fake = notary("Accepted", "Signature size=9000\n", "");
    let failure = call(&set, &fake, &["--app", "dist/Ledger.app", "--keychain-profile", "ci"]).unwrap_err();
    assert!(failure.message.contains("is not signed with the hardened runtime"));
    let adhoc = notary("Accepted", "Signature=adhoc\n", "");
    let failure = call(&set, &adhoc, &["--app", "dist/Ledger.app", "--keychain-profile", "ci"]).unwrap_err();
    assert!(failure.message.contains("carries an ad hoc signature"));
}

#[test]
fn credentials_must_be_given_once_and_whole() {
    let set = setup("notarize-flags");
    let fake = notary("Accepted", DETAILS, "");
    let none = call(&set, &fake, &["--app", "dist/Ledger.app"]).unwrap_err();
    assert!(none.message.starts_with("pass --keychain-profile or --key with --key-id and --issuer;"));
    let part = call(&set, &fake, &["--app", "dist/Ledger.app", "--key", "k"]).unwrap_err();
    assert!(part.message.starts_with("--key, --key-id and --issuer go together"));
    let both = call(&set, &fake, &["--app", "dist/Ledger.app", "--keychain-profile", "p", "--key-id", "k"]).unwrap_err();
    assert!(both.message.contains("not both"));
    let other = call(&set, &fake, &["--app", "dist/Other.app", "--keychain-profile", "p"]).unwrap_err();
    assert!(other.message.contains("is not a bundle directory"));
    assert!(fake.calls.borrow().is_empty());
}
