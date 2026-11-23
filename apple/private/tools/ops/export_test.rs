mod fake;

use std::path::Path;

use dx_apple_ops::export::run;
use dx_apple_ops::export::{build_version, listing, now};
use dx_apple_ops::util::{Cmd, Output};
use dx_apple_ops::xcode::{CODESIGN, DITTO, OPENSSL, PLUTIL, SECURITY, Tools, XCRUN};
use dx_test_scratch::{scratch, write};
use fake::{Fake, args, env, fail, ok};

const SHA1: &str = "0A1B2C3D4E5F60718293A4B5C6D7E8F901234567";
const DETAILS: &str =
    "Authority=Apple Distribution: Ledger (TEAM123456)\nTeamIdentifier=TEAM123456\n";
const OTOOL: &str =
    "Load command 9\n      cmd LC_BUILD_VERSION\n  cmdsize 32\n platform 2\n    minos 17.0\n";

struct Setup {
    root: dx_test_scratch::TempDir,
    tmp: String,
    ipa: String,
    profile: String,
    out: String,
}

fn setup(prefix: &str, with_extension: bool) -> Setup {
    let root = scratch(prefix).unwrap();
    write(root.path(), "profile.mobileprovision", "profile-bytes").unwrap();
    write(root.path(), "id.p12", "p12").unwrap();
    write(root.path(), "pass", "secret").unwrap();
    write(root.path(), "out/keep", "").unwrap();
    write(root.path(), "tmp/keep", "").unwrap();
    let ipa = root.path().join("Ledger.ipa");
    let mut writer = dx_archive::ZipWriter::new();
    let mut entries = vec![
        ("Payload/Ledger.app/Info.plist", "plist"),
        ("Payload/Ledger.app/Ledger", "binary"),
        (
            "Payload/Ledger.app/embedded.mobileprovision",
            "profile-bytes",
        ),
    ];
    if with_extension {
        entries.push(("Payload/Ledger.app/PlugIns/Share.appex/Info.plist", "x"));
    }
    for (name, text) in entries {
        writer.add_file(name, text.as_bytes(), 0o644).unwrap();
    }
    std::fs::write(&ipa, writer.finish().unwrap()).unwrap();
    Setup {
        tmp: root.path().join("tmp").to_string_lossy().into_owned(),
        ipa: ipa.to_string_lossy().into_owned(),
        profile: root
            .path()
            .join("profile.mobileprovision")
            .to_string_lossy()
            .into_owned(),
        out: root
            .path()
            .join("out/Ledger.ipa")
            .to_string_lossy()
            .into_owned(),
        root,
    }
}

fn devices(expires: &'static str, identifier: &'static str, extra_entitlement: bool) -> Fake {
    Fake::new(move |cmd: &Cmd| {
        let arg = |i: usize| cmd.args.get(i).cloned().unwrap_or_default();
        match cmd.program.as_str() {
            SECURITY if arg(0) == "cms" => ok("decoded"),
            SECURITY if arg(0) == "find-identity" => {
                ok(&format!("  1) {SHA1} \"Apple Distribution\"\n"))
            }
            SECURITY => ok(""),
            PLUTIL if arg(0) == "-extract" => match arg(1).as_str() {
                "TeamIdentifier.0" => ok("TEAM123456\n"),
                "Name" => ok("Ledger Profile\n"),
                "UUID" => ok("uuid-1\n"),
                "ExpirationDate" => ok(&format!("{expires}\n")),
                "Entitlements.application-identifier" => ok(&format!("{identifier}\n")),
                "DeveloperCertificates.0" => ok("AAAA\n"),
                "CFBundleExecutable" => ok("Ledger\n"),
                "Entitlements" => {
                    ok("<dict>\n<key>application-identifier</key>\n<string>x</string>\n</dict>\n")
                }
                _ => fail(1, ""),
            },
            PLUTIL => ok(""),
            OPENSSL if arg(0) == "rand" => ok("aabbcc\n"),
            OPENSSL if arg(0) == "pkcs12" => {
                if let Some(at) = cmd.args.iter().position(|a| a == "-out") {
                    std::fs::write(&cmd.args[at + 1], "data").unwrap();
                }
                ok("")
            }
            OPENSSL => ok(&format!(
                "sha1 Fingerprint={}\n",
                SHA1.as_bytes()
                    .chunks(2)
                    .map(|c| std::str::from_utf8(c).unwrap())
                    .collect::<Vec<_>>()
                    .join(":")
            )),
            CODESIGN if arg(0) == "-d" => {
                if extra_entitlement {
                    ok(
                        "<dict>\n<key>application-identifier</key>\n<string>x</string>\n<key>aps-environment</key>\n<string>y</string>\n</dict>\n",
                    )
                } else {
                    ok("")
                }
            }
            CODESIGN if arg(0) == "-dvv" => Output {
                code: Some(0),
                stdout: String::new(),
                stderr: DETAILS.to_owned(),
            },
            CODESIGN => ok(""),
            DITTO => {
                let payload = Path::new(&arg(4)).to_path_buf();
                let target = Path::new(&arg(5)).to_path_buf();
                let mut writer = dx_archive::ZipWriter::new();
                for name in ["Info.plist", "Ledger", "embedded.mobileprovision"] {
                    assert!(payload.join("Ledger.app").join(name).exists());
                    writer
                        .add_file(&format!("Payload/Ledger.app/{name}"), b"", 0o644)
                        .unwrap();
                }
                std::fs::write(&target, writer.finish().unwrap()).unwrap();
                ok("")
            }
            XCRUN if arg(0) == "lipo" => ok("arm64\n"),
            XCRUN if arg(0) == "otool" => ok(OTOOL),
            _ => ok(""),
        }
    })
}

fn call(set: &Setup, fake: &Fake, extra: &[&str]) -> dx_apple_ops::util::Outcome {
    let mut all = vec!["com.example.ledger", set.ipa.as_str(), set.profile.as_str()];
    all.extend([
        "--certificate",
        "id.p12",
        "--password-file",
        "pass",
        "--out",
        &set.out,
    ]);
    all.extend(extra);
    let tools = Tools {
        developer_dir: None,
        runner: fake,
    };
    let pairs = [
        ("TMPDIR", set.tmp.as_str()),
        (
            "BUILD_WORKING_DIRECTORY",
            &set.root.path().to_string_lossy(),
        ),
        ("DX_NOW", "2026-11-10T00:00:00Z"),
    ];
    run(&tools, &args(&all), &env(&pairs))
}

#[test]
fn a_device_app_is_signed_into_an_ipa_and_the_report_is_written() {
    let set = setup("export-ok", false);
    let fake = devices("2027-01-01T00:00:00Z", "TEAM123456.com.example.*", false);
    assert_eq!(call(&set, &fake, &[]), Ok(0));
    assert!(Path::new(&set.out).is_file());
    let report = std::fs::read_to_string(format!("{}.txt", set.out)).unwrap();
    assert!(report.starts_with("bundle_id=com.example.ledger\nteam=TEAM123456\nprofile_name=Ledger Profile\nprofile_uuid=uuid-1\n"));
    assert!(report.ends_with("archs=arm64\nplatform=iphoneos\nminos=17.0\n"));
    let log = fake.log();
    assert!(
        log.iter()
            .any(|l| l.starts_with("--force --keychain") && l.contains(&format!("--sign {SHA1}")))
    );
    assert!(log.iter().any(|l| l.starts_with("delete-keychain")));
    assert!(
        std::fs::read_dir(&set.tmp)
            .unwrap()
            .all(|e| e.unwrap().file_name() == "keep")
    );
}

#[test]
fn an_expired_profile_is_refused() {
    let set = setup("export-expired", false);
    let fake = devices(
        "2026-01-01T00:00:00Z",
        "TEAM123456.com.example.ledger",
        false,
    );
    let failure = call(&set, &fake, &[]).unwrap_err();
    assert!(failure.message.contains("expired on 2026-01-01T00:00:00Z"));
}

#[test]
fn a_profile_for_another_app_is_refused() {
    let set = setup("export-other", false);
    let fake = devices("2027-01-01T00:00:00Z", "TEAM123456.com.other.app", false);
    let failure = call(&set, &fake, &[]).unwrap_err();
    assert!(
        failure
            .message
            .contains("covers TEAM123456.com.other.app, not TEAM123456.com.example.ledger")
    );
}

#[test]
fn an_entitlement_the_profile_does_not_allow_is_refused() {
    let set = setup("export-entitlement", false);
    let fake = devices(
        "2027-01-01T00:00:00Z",
        "TEAM123456.com.example.ledger",
        true,
    );
    let failure = call(&set, &fake, &[]).unwrap_err();
    assert!(
        failure
            .message
            .starts_with("entitlement 'aps-environment' is not allowed")
    );
    assert!(!fake.log().iter().any(|l| l.starts_with("create-keychain")));
}

#[test]
fn app_extensions_are_refused() {
    let set = setup("export-extension", true);
    let fake = devices(
        "2027-01-01T00:00:00Z",
        "TEAM123456.com.example.ledger",
        false,
    );
    let failure = call(&set, &fake, &[]).unwrap_err();
    assert!(failure.message.contains("app extensions"));
}

#[test]
fn an_ipa_with_an_escaping_symlink_is_refused() {
    let set = setup("export-escape", false);
    let mut writer = dx_archive_testing::RawZip::new();
    writer
        .add_file("Payload/Ledger.app/Info.plist", b"plist")
        .unwrap();
    writer
        .add_symlink("Payload/Ledger.app/escape", "../../../outside")
        .unwrap();
    std::fs::write(&set.ipa, writer.finish().unwrap()).unwrap();
    let fake = devices(
        "2027-01-01T00:00:00Z",
        "TEAM123456.com.example.ledger",
        false,
    );
    let failure = call(&set, &fake, &[]).unwrap_err();
    assert_eq!(failure.code, 2);
    assert!(
        failure.message.starts_with("cannot unpack"),
        "{}",
        failure.message
    );
    assert!(!Path::new(&set.out).exists());
}

#[test]
fn flags_and_outputs_are_checked() {
    let set = setup("export-flags", false);
    let fake = devices(
        "2027-01-01T00:00:00Z",
        "TEAM123456.com.example.ledger",
        false,
    );
    let failure = call(&set, &fake, &["--out", "out/Ledger.zip"]).unwrap_err();
    assert!(failure.message.contains("must name an .ipa file"));
    let tools = Tools {
        developer_dir: None,
        runner: &fake,
    };
    let missing = run(
        &tools,
        &args(&["com.example.ledger", "a", "b", "--out", "x.ipa"]),
        &env(&[]),
    )
    .unwrap_err();
    assert!(
        missing
            .message
            .starts_with("--certificate names the signing identity .p12 file")
    );
    assert!(fake.calls.borrow().is_empty());
}

#[test]
fn the_ipa_listing_skips_directories_and_signatures() {
    let set = setup("export-listing", false);
    let names = listing(Path::new(&set.ipa)).unwrap();
    assert_eq!(names.len(), 3);
    assert!(names.contains("Payload/Ledger.app/Info.plist"));
}

#[test]
fn the_build_version_is_read_from_the_first_build_command() {
    assert_eq!(build_version(OTOOL), ("2".to_owned(), "17.0".to_owned()));
    assert_eq!(
        build_version("cmd LC_SEGMENT\n platform 9\n"),
        (String::new(), String::new())
    );
}

#[test]
fn the_current_time_has_the_profile_format() {
    let stamp = now();
    assert_eq!(stamp.len(), 20);
    assert!(stamp.starts_with("20") && stamp.ends_with('Z'));
}
