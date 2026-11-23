use dx_android_ops::util::Captured;
use dx_android_ops::{adb, apk_set, check, sign};

fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

#[test]
fn adb_flags_let_the_last_one_win() {
    let flags = adb::parse(&strings(&["--adb=a", "--serial=s", "--op=log", "--adb=b"])).unwrap();
    assert_eq!(flags.adb, "b");
    assert_eq!(flags.serial, "s");
    assert_eq!(
        adb::parse(&strings(&["--bogus=1"])),
        Err("unknown argument '--bogus=1'".to_owned())
    );
    assert_eq!(
        adb::parse(&strings(&["install"])),
        Err("unknown argument 'install'".to_owned())
    );
}

#[test]
fn adb_args_name_each_operation() {
    let flags = |values: &[&str]| adb::parse(&strings(values)).unwrap();
    assert_eq!(
        adb::adb_args(&flags(&["--serial=e", "--op=install", "--apk=a.apk"])),
        Ok(strings(&["-s", "e", "install", "-r", "a.apk"]))
    );
    assert_eq!(
        adb::adb_args(&flags(&[
            "--serial=e",
            "--op=start",
            "--app=a.b",
            "--activity=C"
        ])),
        Ok(strings(&["-s", "e", "shell", "am", "start", "-n", "a.b/C"]))
    );
    assert_eq!(
        adb::adb_args(&flags(&["--serial=e", "--op=log"])),
        Ok(strings(&["-s", "e", "logcat", "-d"]))
    );
}

#[test]
fn adb_args_reject_an_incomplete_operation() {
    let error =
        |values: &[&str]| adb::adb_args(&adb::parse(&strings(values)).unwrap()).unwrap_err();
    assert_eq!(
        error(&["--op=log"]),
        "--serial names the target device or emulator"
    );
    assert_eq!(
        error(&["--serial=e", "--op=install"]),
        "--apk names the APK file"
    );
    assert_eq!(
        error(&[
            "--serial=e",
            "--op=install-multiple",
            "--apks=/nonexistent/dx"
        ]),
        "--apks names the APK set directory"
    );
    assert_eq!(
        error(&["--serial=e", "--op=start", "--app=a.b"]),
        "--app and --activity name the component"
    );
    assert_eq!(error(&["--serial=e", "--op=wipe"]), "unknown op 'wipe'");
}

#[test]
fn apk_set_reason_reads_the_first_bundletool_error() {
    let log = "note\n[BT:1.18.1] Error: no match\n[BT:1.18.1] Error: second\n";
    assert_eq!(apk_set::reason(log), "no match");
    assert_eq!(apk_set::reason("plain failure"), "");
}

#[test]
fn check_flags_collect_abis_and_resources() {
    let wants = check::parse(&strings(&[
        "--abi",
        "x86_64",
        "--resource",
        "string/a",
        "--abi",
        "arm64-v8a",
    ]))
    .unwrap();
    assert_eq!(wants.abis, strings(&["x86_64", "arm64-v8a"]));
    assert_eq!(wants.resources, strings(&["string/a"]));
    assert_eq!(
        check::parse(&strings(&["--abi"])),
        Err("--abi needs an ABI name".to_owned())
    );
    assert_eq!(
        check::parse(&strings(&["--resource", ""])),
        Err("--resource needs a type/name".to_owned())
    );
    assert_eq!(
        check::parse(&strings(&["--other"])),
        Err("unknown argument '--other'".to_owned())
    );
}

#[test]
fn check_names_signature_entries_and_base_libraries() {
    for name in [
        "META-INF/CERT.RSA",
        "META-INF/CERT.SF",
        "META-INF/A.DSA",
        "META-INF/A.EC",
    ] {
        assert!(check::signature_entry(name), "{name}");
    }
    for name in [
        "META-INF/MANIFEST.MF",
        "META-INF/a/CERT.RSA",
        "META-INF/.RSA",
        "lib/CERT.RSA",
    ] {
        assert!(!check::signature_entry(name), "{name}");
    }
    assert!(check::base_library("base/lib/x86_64/libapp.so", "x86_64"));
    assert!(!check::base_library("base/lib/x86_64/libapp.so", "x86"));
    assert!(!check::base_library(
        "base/lib/x86_64/deep/libapp.so",
        "x86_64"
    ));
    assert!(!check::base_library(
        "feature/lib/x86_64/libapp.so",
        "x86_64"
    ));
    assert!(!check::base_library("base/lib/x86_64/.so", "x86_64"));
}

#[test]
fn check_reads_every_entry_back() {
    let dir = std::path::PathBuf::from(std::env::var("TEST_TMPDIR").unwrap());
    let good = dir.join("good.zip");
    let mut writer = dx_archive_testing::RawZip::new();
    writer
        .add_file("AndroidManifest.xml", b"manifest bytes")
        .unwrap();
    writer.add_dir("META-INF").unwrap();
    std::fs::write(&good, writer.finish().unwrap()).unwrap();
    assert!(check::entries_intact(&good));
    let mut bytes = std::fs::read(&good).unwrap();
    let at = bytes
        .windows(8)
        .position(|window| window == b"manifest")
        .unwrap();
    bytes[at] ^= 0xff;
    let bad = dir.join("bad.zip");
    std::fs::write(&bad, bytes).unwrap();
    assert!(!check::entries_intact(&bad));
    assert!(!check::entries_intact(&dir.join("missing.zip")));
}

#[test]
fn check_refuses_a_dot_dot_member() {
    let dir = std::path::PathBuf::from(std::env::var("TEST_TMPDIR").unwrap());
    let package = dir.join("up.zip");
    let mut writer = dx_archive_testing::RawZip::new();
    writer
        .add_file("../AndroidManifest.xml", b"manifest bytes")
        .unwrap();
    std::fs::write(&package, writer.finish().unwrap()).unwrap();
    assert!(!check::entries_intact(&package));
}

#[test]
fn check_rejects_a_bad_invocation_with_exit_code_two() {
    let failure = check::run(&[]).unwrap_err();
    assert_eq!(failure.code, 2);
    assert!(
        failure.message.starts_with("usage: check "),
        "{}",
        failure.message
    );
    let failure = check::run(&strings(&["/nonexistent/bundletool", "a.apk"])).unwrap_err();
    assert_eq!(
        (failure.code, failure.message.as_str()),
        (2, "no bundletool at /nonexistent/bundletool")
    );
}

#[test]
fn sign_flags_let_the_last_one_win() {
    let args = strings(&["--java=a", "--java-extra=x", "--java=b", "--schemes=v2,v3"]);
    assert_eq!(sign::flag(&args, "java"), "b");
    assert_eq!(sign::flag(&args, "schemes"), "v2,v3");
    assert_eq!(sign::flag(&args, "zipalign"), "");
}

#[test]
fn sign_schemes_turn_each_apksigner_flag_on_or_off() {
    assert_eq!(
        sign::scheme_flags(&["v1", "v3"]),
        strings(&[
            "--v1-signing-enabled",
            "true",
            "--v2-signing-enabled",
            "false",
            "--v3-signing-enabled",
            "true",
            "--v4-signing-enabled",
            "false",
        ])
    );
    assert_eq!(sign::wants_v4(&["v1", "v2"]), Ok(false));
    assert_eq!(sign::wants_v4(&["v2", "v4"]), Ok(true));
    assert_eq!(sign::wants_v4(&["v3", "v4"]), Ok(true));
    assert_eq!(
        sign::wants_v4(&["v1", "v4"]),
        Err("scheme v4 needs v2 or v3")
    );
}

#[test]
fn sign_picks_the_algorithm_of_the_upload_key() {
    assert_eq!(
        sign::signature_algorithm(
            "Alias name: a\nSubject Public Key Algorithm: 2048-bit RSA key\n"
        ),
        Some("SHA256withRSA")
    );
    assert_eq!(
        sign::signature_algorithm("Subject Public Key Algorithm: 256-bit EC (secp256r1) key\n"),
        Some("SHA256withECDSA")
    );
    assert_eq!(
        sign::signature_algorithm("Subject Public Key Algorithm: 2048-bit DSA key\n"),
        None
    );
    assert_eq!(sign::signature_algorithm("nothing"), None);
}

#[test]
fn sign_accepts_only_a_verified_bundle() {
    let ran = |code: i32, text: &str| Captured {
        code: Some(code),
        text: text.to_owned(),
    };
    let plain = Some("signed App Bundle failed verification");
    assert_eq!(sign::verify_error(&ran(0, "jar verified.\n")), None);
    assert_eq!(sign::verify_error(&ran(0, "jar is unsigned.\n")), plain);
    assert_eq!(
        sign::verify_error(&ran(4, "jar verified, with signer errors.\nself-signed\n")),
        None
    );
    assert_eq!(sign::verify_error(&ran(4, "jar verified.\n")), plain);
    assert_eq!(
        sign::verify_error(&ran(
            4,
            "jar verified.\nself-signed\ncertificate has expired\n"
        )),
        Some("signed App Bundle failed verification: the certificate is expired or not yet valid")
    );
    assert_eq!(sign::verify_error(&ran(1, "jar verified.\n")), plain);
}
