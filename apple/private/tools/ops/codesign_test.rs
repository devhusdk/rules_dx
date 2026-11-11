//! Tests signing a built macOS app.

mod fake;

use std::path::{Path, PathBuf};

use dx_apple_ops::codesign::run;
use dx_apple_ops::util::{Cmd, Output};
use dx_apple_ops::xcode::{CODESIGN, DITTO, OPENSSL, PLUTIL, SECURITY, Tools};
use dx_test_scratch::{scratch, write};
use fake::{Fake, args, env, fail, ok};

const MACHO: &[u8] = &[0xcf, 0xfa, 0xed, 0xfe, 0, 0, 0, 0];
const DETAILS: &str = "CodeDirectory v=20500 size=1 flags=0x10000(runtime) hashes=1\nAuthority=Developer ID Application: Ledger (TEAM123456)\nTeamIdentifier=TEAM123456\nTimestamp=Jan 1, 2027 at 10:00:00\n";
const IDENTITIES: &str = "  1) 0A1B2C3D4E5F60718293A4B5C6D7E8F901234567 \"Developer ID Application: Ledger (TEAM123456)\"\n     1 valid identities found\n";

struct Setup {
    root: dx_test_scratch::TempDir,
    tmp: String,
    out: String,
    archive: String,
}

fn setup(prefix: &str) -> Setup {
    let root = scratch(prefix);
    write(root.path(), "Ledger.zip", "PK");
    write(root.path(), "out/keep", "");
    write(root.path(), "tmp/keep", "");
    Setup {
        tmp: root.path().join("tmp").to_string_lossy().into_owned(),
        out: root
            .path()
            .join("out/Ledger.app")
            .to_string_lossy()
            .into_owned(),
        archive: root
            .path()
            .join("Ledger.zip")
            .to_string_lossy()
            .into_owned(),
        root,
    }
}

fn tree(target: &Path, extra: &dyn Fn(&Path)) {
    let contents = target.join("Ledger.app/Contents");
    std::fs::create_dir_all(contents.join("MacOS")).unwrap();
    std::fs::write(contents.join("Info.plist"), "plist").unwrap();
    std::fs::write(contents.join("MacOS/Ledger"), MACHO).unwrap();
    extra(&contents);
}

fn tools_fake(extra: impl Fn(&Path) + 'static, sign_error: &'static str) -> Fake {
    Fake::new(move |cmd: &Cmd| {
        let arg = |i: usize| cmd.args.get(i).cloned().unwrap_or_default();
        match cmd.program.as_str() {
            DITTO if arg(0) == "-x" => {
                tree(Path::new(&arg(3)), &extra);
                ok("")
            }
            DITTO => {
                std::fs::create_dir_all(arg(1)).unwrap();
                ok("")
            }
            PLUTIL if arg(0) == "-extract" => match arg(1).as_str() {
                "CFBundleExecutable" => ok("Ledger\n"),
                _ => fail(1, "missing"),
            },
            OPENSSL if arg(0) == "rand" => ok("aabbcc\n"),
            OPENSSL if arg(0) == "pkcs12" => {
                if let Some(at) = cmd.args.iter().position(|a| a == "-out") {
                    std::fs::write(&cmd.args[at + 1], "data").unwrap();
                }
                ok("")
            }
            OPENSSL => {
                ok("SHA1 Fingerprint=0A:1B:2C:3D:4E:5F:60:71:82:93:A4:B5:C6:D7:E8:F9:01:23:45:67\n")
            }
            SECURITY if arg(0) == "find-identity" => ok(IDENTITIES),
            SECURITY => ok(""),
            CODESIGN if arg(0) == "-dvv" => Output {
                code: Some(0),
                stdout: String::new(),
                stderr: DETAILS.to_owned(),
            },
            CODESIGN if arg(0) == "-d" => ok(""),
            CODESIGN if !sign_error.is_empty() && arg(0) == "--force" => fail(1, sign_error),
            CODESIGN => ok(""),
            _ => ok("arm64\n"),
        }
    })
}

fn tools(fake: &Fake) -> Tools<'_> {
    Tools {
        developer_dir: None,
        runner: fake,
    }
}

fn call(set: &Setup, fake: &Fake, flags: &[&str]) -> dx_apple_ops::util::Outcome {
    let mut all = vec!["com.example.ledger", set.archive.as_str(), ""];
    all.extend(flags);
    run(
        &tools(fake),
        &args(&all),
        &env(&[
            ("TMPDIR", &set.tmp),
            (
                "BUILD_WORKING_DIRECTORY",
                &set.root.path().to_string_lossy(),
            ),
        ]),
    )
}

fn signs(fake: &Fake) -> Vec<Vec<String>> {
    fake.calls
        .borrow()
        .iter()
        .filter(|c| c.program == CODESIGN && c.args[0] == "--force")
        .map(|c| c.args.clone())
        .collect()
}

#[test]
fn an_identity_signs_the_app_with_the_hardened_runtime_and_writes_the_report() {
    let set = setup("sign-identity");
    let fake = tools_fake(|_| {}, "");
    let outcome = call(&set, &fake, &["--identity", "Ledger", "--out", &set.out]);
    assert_eq!(outcome, Ok(0));
    let steps = signs(&fake);
    assert_eq!(steps.len(), 1);
    assert_eq!(
        &steps[0][..7],
        [
            "--force",
            "--options",
            "runtime",
            "--timestamp",
            "--sign",
            "0A1B2C3D4E5F60718293A4B5C6D7E8F901234567",
            steps[0][6].as_str()
        ]
    );
    let report = std::fs::read_to_string(format!("{}.txt", set.out)).unwrap();
    assert!(report.contains("bundle_id=com.example.ledger\nteam=TEAM123456\n"));
    assert!(report.contains("certificate_sha1=0A1B2C3D4E5F60718293A4B5C6D7E8F901234567\n"));
    assert!(report.ends_with("archs=arm64\nruntime=yes\n"));
    assert!(!fake.log().iter().any(|l| l.starts_with("create-keychain")));
}

#[test]
fn a_certificate_signs_from_a_throwaway_keychain_that_is_removed() {
    let set = setup("sign-certificate");
    write(set.root.path(), "id.p12", "p12");
    write(set.root.path(), "pass", "secret");
    let fake = tools_fake(|_| {}, "");
    let outcome = call(
        &set,
        &fake,
        &[
            "--certificate",
            "id.p12",
            "--password-file",
            "pass",
            "--out",
            &set.out,
        ],
    );
    assert_eq!(outcome, Ok(0));
    let log = fake.log();
    assert!(log.iter().any(|l| l.starts_with("create-keychain -p")));
    let steps = signs(&fake);
    assert!(steps[0].contains(&"--keychain".to_owned()));
    assert!(log.iter().any(|l| l.starts_with("delete-keychain")));
    assert!(
        !PathBuf::from(&set.tmp)
            .read_dir()
            .unwrap()
            .any(|e| e.unwrap().file_name() != "keep")
    );
}

#[test]
fn nested_code_is_signed_before_the_app_and_keeps_its_entitlements() {
    let set = setup("sign-nested");
    let fake = tools_fake(
        |contents| {
            std::fs::create_dir_all(contents.join("Frameworks/Kit.framework")).unwrap();
            std::fs::write(contents.join("Frameworks/Kit.framework/Kit"), MACHO).unwrap();
            std::fs::create_dir_all(contents.join("Helpers")).unwrap();
            std::fs::write(contents.join("Helpers/tool"), MACHO).unwrap();
        },
        "",
    );
    let outcome = call(&set, &fake, &["--identity", "Ledger", "--out", &set.out]);
    assert_eq!(outcome, Ok(0));
    let steps = signs(&fake);
    assert_eq!(steps.len(), 3);
    assert!(
        steps[0]
            .last()
            .unwrap()
            .ends_with("Frameworks/Kit.framework")
    );
    assert!(steps[1].contains(&"--preserve-metadata=entitlements".to_owned()));
    assert!(steps[2].last().unwrap().ends_with("Ledger.app"));
}

#[test]
fn code_outside_the_signed_places_is_refused() {
    let set = setup("sign-stray");
    let fake = tools_fake(
        |contents| {
            std::fs::create_dir_all(contents.join("Resources")).unwrap();
            std::fs::write(contents.join("Resources/stray"), MACHO).unwrap();
        },
        "",
    );
    let outcome = call(&set, &fake, &["--identity", "Ledger", "--out", &set.out]);
    let failure = outcome.unwrap_err();
    assert_eq!(failure.code, 2);
    assert!(
        failure
            .message
            .contains("Contents/Resources/stray outside the places this launcher signs")
    );
    assert!(signs(&fake).is_empty());
}

#[test]
fn a_debuggable_helper_is_refused() {
    let set = setup("sign-debuggable");
    let fake = Fake::new(|cmd: &Cmd| match cmd.program.as_str() {
        CODESIGN if cmd.args[0] == "-d" && cmd.args.last().unwrap().ends_with("tool") => {
            ok("<key>com.apple.security.get-task-allow</key>\n<true/>")
        }
        DITTO => {
            tree(Path::new(&cmd.args[3]), &|contents| {
                std::fs::create_dir_all(contents.join("Helpers")).unwrap();
                std::fs::write(contents.join("Helpers/tool"), MACHO).unwrap();
            });
            ok("")
        }
        PLUTIL => ok("Ledger\n"),
        _ => ok(""),
    });
    let failure = call(&set, &fake, &["--identity", "Ledger", "--out", &set.out]).unwrap_err();
    assert!(
        failure
            .message
            .contains("Contents/Helpers/tool sets entitlement 'com.apple.security.get-task-allow'")
    );
}

#[test]
fn an_ambiguous_identity_asks_for_the_sha1() {
    let set = setup("sign-ambiguous");
    let two = format!(
        "{IDENTITIES}  2) FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF \"Developer ID Application: Ledger Two (TEAM123456)\"\n"
    );
    let fake = Fake::new(move |cmd: &Cmd| match cmd.program.as_str() {
        SECURITY => ok(&two),
        DITTO => {
            tree(Path::new(&cmd.args[3]), &|_| {});
            ok("")
        }
        PLUTIL => ok("Ledger\n"),
        _ => ok(""),
    });
    let failure = call(&set, &fake, &["--identity", "Ledger", "--out", &set.out]).unwrap_err();
    assert_eq!(
        failure.message,
        "--identity Ledger matches more than one code signing identity; pass its SHA-1"
    );
}

#[test]
fn a_missing_timestamp_service_exits_with_code_three() {
    let set = setup("sign-offline");
    let fake = tools_fake(|_| {}, "The timestamp service is not available.");
    let failure = call(&set, &fake, &["--identity", "Ledger", "--out", &set.out]).unwrap_err();
    assert_eq!(failure.code, 3);
}

#[test]
fn bad_flags_are_refused_before_any_tool_runs() {
    let set = setup("sign-flags");
    let fake = tools_fake(|_| {}, "");
    let both = call(
        &set,
        &fake,
        &["--identity", "x", "--certificate", "c", "--out", &set.out],
    )
    .unwrap_err();
    assert!(
        both.message
            .starts_with("pass --identity or --certificate with --password-file, not both")
    );
    let suffix = call(&set, &fake, &["--identity", "x", "--out", "out/Ledger"]).unwrap_err();
    assert!(suffix.message.contains("must name an .app bundle"));
    let keychain = call(
        &set,
        &fake,
        &["--keychain", "k.keychain", "--out", &set.out],
    )
    .unwrap_err();
    assert!(
        keychain
            .message
            .starts_with("--keychain only goes with --identity")
    );
    let unknown = call(&set, &fake, &["--nope"]).unwrap_err();
    assert!(unknown.message.starts_with("unexpected argument"));
    assert!(fake.calls.borrow().is_empty());
}
