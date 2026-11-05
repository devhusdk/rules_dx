use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use dx_testing::{mkscratch, resolve_runfiles, run, Run};

const FAKE_SECURITY: &str = r#"#!/bin/sh
printf 'security %s\n' "$*" >>"$FAKE_LOG"
case "$1" in
    cms)
        cat "$4"
        ;;
    list-keychains)
        if [ "${4:-}" = "-s" ]; then
            shift 4
            : >"$FAKE_KEYCHAINS"
            for keychain in "$@"; do
                printf '    "%s"\n' "$keychain" >>"$FAKE_KEYCHAINS"
            done
        else
            cat "$FAKE_KEYCHAINS"
        fi
        ;;
    find-identity)
        printf '  1) %s "Apple Development: Ledger (ABCDE12345)"\n' "$FAKE_IDENTITY"
        ;;
esac
"#;

const FAKE_OPENSSL: &str = r#"#!/bin/sh
printf 'openssl %s\n' "$*" >>"$FAKE_LOG"
case "$1" in
    rand)
        echo 5f0c7e1a9b2d4c6e8f00112233445566778899aabbccddee
        ;;
    pkcs12)
        export_out=""
        nokeys=""
        password_ok=1
        while [ $# -gt 0 ]; do
            case "$1" in
                -passin)
                    [ "$(cat "${2#file:}")" = "$FAKE_PASSWORD" ] || password_ok=""
                    shift
                    ;;
                -out) export_out="$2"; shift ;;
                -nokeys) nokeys=1 ;;
            esac
            shift
        done
        if [ -n "$export_out" ]; then
            [ "$(cat)" = "KEY" ] || exit 1
            echo p12 >"$export_out"
            exit 0
        fi
        [ -n "$password_ok" ] || exit 1
        if [ -n "$nokeys" ]; then
            printf '%s' "$FAKE_CERT"
        else
            printf 'KEY'
        fi
        ;;
    x509)
        printf 'SHA1 Fingerprint=%s\n' "$(cat)"
        ;;
    base64)
        cat
        ;;
esac
"#;

const FAKE_PLUTIL: &str = r#"#!/bin/sh
printf 'plutil %s\n' "$*" >>"$FAKE_LOG"
case "$1" in
    -extract)
        if [ "$3" = "xml1" ]; then
            grep -q "^$2\." "$6" || exit 1
            printf '<?xml version="1.0" encoding="UTF-8"?>\n<plist version="1.0">\n<dict>\n'
            sed -n "s/^$2\.\([^=]*\)=.*/\1/p" "$6" | while read -r key; do
                printf '\t<key>%s</key>\n\t<true/>\n' "$key"
            done
            printf '</dict>\n</plist>\n'
        else
            line=$(grep -m 1 "^$2=" "$6") || exit 1
            printf '%s\n' "${line#*=}"
        fi
        ;;
esac
"#;

const FAKE_CODESIGN: &str = r#"#!/bin/sh
printf 'codesign %s\n' "$*" >>"$FAKE_LOG"
for last; do :; done
case "$1" in
    -d*)
        if [ "$2" = "--entitlements" ]; then
            cat "$FAKE_ENTITLEMENTS"
            exit 0
        fi
        printf 'Executable=%s\nAuthority=Apple Development: Ledger (ABCDE12345)\nTeamIdentifier=%s\n' "$last" "${FAKE_SIGNED_TEAM:-ABCDE12345}" >&2
        ;;
    --verify)
        exit "${FAKE_VERIFY_EXIT:-0}"
        ;;
    --force)
        if [ "${FAKE_SIGN_EXIT:-0}" != 0 ]; then
            echo "codesign: errSecInternalComponent" >&2
            exit "$FAKE_SIGN_EXIT"
        fi
        if [ -d "$last" ]; then
            mkdir -p "$last/_CodeSignature"
            echo signed >"$last/_CodeSignature/CodeResources"
        fi
        if [ -n "${FAKE_DROP:-}" ] && [ -e "$last/$FAKE_DROP" ]; then
            rm -f "$last/$FAKE_DROP"
        fi
        ;;
esac
"#;

const FAKE_XCRUN: &str = r#"#!/bin/sh
printf 'xcrun %s\n' "$*" >>"$FAKE_LOG"
case "$1" in
    lipo)
        printf '%s\n' "${FAKE_ARCHS:-arm64}"
        ;;
    otool)
        printf 'Load command 9\n      cmd LC_BUILD_VERSION\n  cmdsize 32\n platform %s\n    minos 17.0\n      sdk 18.5\n' "${FAKE_PLATFORM:-2}"
        ;;
esac
"#;

const PROFILE: &str = "Name=Ledger Development
UUID=5D1A6C2E-8B3F-4A9D-9E0C-7F1B2A3C4D5E
TeamIdentifier.0=ABCDE12345
ExpirationDate=2030-01-01T00:00:00Z
Entitlements.application-identifier=ABCDE12345.com.example.ledger
Entitlements.com.apple.developer.team-identifier=ABCDE12345
Entitlements.get-task-allow=true
DeveloperCertificates.0=00:11:22:33
DeveloperCertificates.1=ab:cd:ef:01
";

const ENTITLEMENTS: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0">
<dict>
	<key>application-identifier</key>
	<string>ABCDE12345.com.example.ledger</string>
	<key>get-task-allow</key>
	<true/>
</dict>
</plist>
"#;

const PASSWORD: &str = "hunter2-p12-secret";

const KEYCHAINS: &str = "    \"/Users/dev/Library/Keychains/login.keychain-db\"\n    \"/Library/Keychains/System.keychain\"\n";

const NOW: &str = "2026-10-26T12:00:00Z";

struct Fixture {
    root: PathBuf,
    tools: Vec<(String, String)>,
}

fn write(path: &Path, text: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap_or_else(|error| panic!("mkdir: {error}"));
    }
    std::fs::write(path, text).unwrap_or_else(|error| panic!("write {}: {error}", path.display()));
}

fn tool(root: &Path, name: &str, body: &str) -> String {
    let path = root.join("bin").join(name);
    write(&path, body);
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
        .unwrap_or_else(|error| panic!("chmod: {error}"));
    path.to_string_lossy().into_owned()
}

fn text(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

impl Fixture {
    fn new(prefix: &str) -> Fixture {
        Fixture::with_profile(prefix, PROFILE, PROFILE)
    }

    fn with_profile(prefix: &str, declared: &str, embedded: &str) -> Fixture {
        let root = mkscratch(prefix).unwrap_or_else(|error| panic!("scratch: {error}"));
        let root =
            std::fs::canonicalize(root).unwrap_or_else(|error| panic!("canonicalize: {error}"));
        let tools = vec![
            (
                "SECURITY".to_owned(),
                tool(&root, "security", FAKE_SECURITY),
            ),
            ("OPENSSL".to_owned(), tool(&root, "openssl", FAKE_OPENSSL)),
            ("PLUTIL".to_owned(), tool(&root, "plutil", FAKE_PLUTIL)),
            (
                "CODESIGN".to_owned(),
                tool(&root, "codesign", FAKE_CODESIGN),
            ),
            ("XCRUN".to_owned(), tool(&root, "xcrun", FAKE_XCRUN)),
        ];
        let app = root.join("payload/Payload/Ledger.app");
        write(&app.join("Info.plist"), "CFBundleExecutable=Ledger\n");
        write(&app.join("Ledger"), "binary");
        write(&app.join("Assets.car"), "assets");
        write(&app.join("embedded.mobileprovision"), embedded);
        write(&app.join("_CodeSignature/CodeResources"), "adhoc");
        write(&app.join("Frameworks/Share.framework/Share"), "framework");
        let zipped = std::process::Command::new("zip")
            .current_dir(root.join("payload"))
            .args(["-qry", "../Ledger.ipa", "Payload"])
            .status()
            .unwrap_or_else(|error| panic!("zip: {error}"));
        assert!(zipped.success());
        write(&root.join("dev.mobileprovision"), declared);
        write(&root.join("entitlements.plist"), ENTITLEMENTS);
        write(&root.join("keychains.txt"), KEYCHAINS);
        write(&root.join("ws/identity.p12"), "p12");
        write(&root.join("ws/password.txt"), PASSWORD);
        std::fs::create_dir_all(root.join("ws/dist")).unwrap_or_else(|error| panic!("{error}"));
        std::fs::create_dir_all(root.join("tmp")).unwrap_or_else(|error| panic!("{error}"));
        std::fs::create_dir_all(
            root.join("ob/execroot/_main/bazel-out/darwin_arm64-fastbuild/bin"),
        )
        .unwrap_or_else(|error| panic!("{error}"));
        std::os::unix::fs::symlink(
            root.join("ob/execroot/_main/bazel-out"),
            root.join("ws/bazel-out"),
        )
        .unwrap_or_else(|error| panic!("symlink: {error}"));
        let meta = format!(
            "bundle_id=com.example.ledger\nipa={}\nprofile={}\n",
            root.join("Ledger.ipa").display(),
            root.join("dev.mobileprovision").display()
        );
        write(&root.join("meta.txt"), &meta);
        Fixture { root, tools }
    }

    fn path(&self, name: &str) -> String {
        self.root.join(name).to_string_lossy().into_owned()
    }

    fn log(&self) -> String {
        text(&self.root.join("log.txt"))
    }

    fn export(&self, args: &[&str], envs: &[(&str, &str)]) -> Run {
        let log = self.path("log.txt");
        let keychains = self.path("keychains.txt");
        let entitlements = self.path("entitlements.plist");
        let meta = self.path("meta.txt");
        let ws = self.path("ws");
        let tmp = self.path("tmp");
        let mut all: Vec<(&str, &str)> = self
            .tools
            .iter()
            .map(|(key, value)| (key.as_str(), value.as_str()))
            .collect();
        all.extend_from_slice(&[
            ("DX_ZIP", "zip"),
            ("FAKE_LOG", log.as_str()),
            ("FAKE_KEYCHAINS", keychains.as_str()),
            ("FAKE_ENTITLEMENTS", entitlements.as_str()),
            ("FAKE_PASSWORD", PASSWORD),
            ("FAKE_CERT", "AB:CD:EF:01"),
            ("FAKE_IDENTITY", "ABCDEF01"),
            ("DX_EXPORT_META", meta.as_str()),
            ("DX_NOW", NOW),
            ("BUILD_WORKSPACE_DIRECTORY", ws.as_str()),
            ("BUILD_WORKING_DIRECTORY", ws.as_str()),
            ("TMPDIR", tmp.as_str()),
        ]);
        all.extend_from_slice(envs);
        let script = resolve_runfiles(
            &std::env::var("DX_EXPORT_OPS")
                .unwrap_or_else(|_| panic!("DX_EXPORT_OPS must name a script")),
        );
        run(&script, args, &all).unwrap_or_else(|error| panic!("run: {error}"))
    }

    fn assert_clean(&self, ran: &Run) {
        assert_eq!(
            text(&self.root.join("keychains.txt")),
            KEYCHAINS,
            "{}",
            ran.combined()
        );
        let leftovers: Vec<_> = std::fs::read_dir(self.root.join("tmp"))
            .unwrap_or_else(|error| panic!("read_dir: {error}"))
            .collect();
        assert!(
            leftovers.is_empty(),
            "the work dir is removed: {}",
            ran.combined()
        );
        for haystack in [
            ran.stdout.as_str(),
            ran.stderr.as_str(),
            self.log().as_str(),
        ] {
            assert!(
                !haystack.contains(PASSWORD),
                "the password never leaks: {haystack}"
            );
        }
    }
}

const ARGS: [&str; 6] = [
    "--certificate",
    "identity.p12",
    "--password-file",
    "password.txt",
    "--out",
    "dist/Ledger.ipa",
];

fn fails(ran: &Run, code: i32, stderr: &str) {
    assert_eq!(ran.status.code(), Some(code), "{}", ran.combined());
    assert_eq!(
        ran.stderr,
        format!("dx-apple-export: {stderr}\n"),
        "{}",
        ran.combined()
    );
}

fn listing(ipa: &Path) -> Vec<String> {
    let out = std::process::Command::new("unzip")
        .args(["-Z1"])
        .arg(ipa)
        .output()
        .unwrap_or_else(|error| panic!("unzip: {error}"));
    let mut names: Vec<String> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter(|line| !line.ends_with('/'))
        .map(str::to_owned)
        .collect();
    names.sort();
    names
}

#[test]
fn export_signs_zips_and_reports_the_device_app() {
    let fixture = Fixture::new("apple-export-ok");
    let ran = fixture.export(&ARGS, &[]);
    assert_eq!(ran.status.code(), Some(0), "{}", ran.combined());
    let out = fixture.root.join("ws/dist/Ledger.ipa");
    assert!(out.is_file(), "{}", ran.combined());
    assert!(listing(&out).contains(&"Payload/Ledger.app/Assets.car".to_owned()));
    assert_eq!(
        text(&fixture.root.join("ws/dist/Ledger.ipa.txt")),
        "bundle_id=com.example.ledger
team=ABCDE12345
profile_name=Ledger Development
profile_uuid=5D1A6C2E-8B3F-4A9D-9E0C-7F1B2A3C4D5E
profile_expires=2030-01-01T00:00:00Z
certificate_sha1=ABCDEF01
authority=Apple Development: Ledger (ABCDE12345)
entitlements=application-identifier,get-task-allow
archs=arm64
platform=iphoneos
minos=17.0
"
    );
    let log = fixture.log();
    let signs: Vec<&str> = log
        .lines()
        .filter(|line| line.starts_with("codesign --force"))
        .collect();
    assert_eq!(signs.len(), 2, "{log}");
    assert!(
        signs[0].ends_with("/Payload/Ledger.app/Frameworks/Share.framework"),
        "{log}"
    );
    assert!(signs[0].contains("--sign ABCDEF01"), "{log}");
    assert!(signs[1].contains("--entitlements "), "{log}");
    assert!(signs[1].ends_with("/Payload/Ledger.app"), "{log}");
    let created = log
        .lines()
        .find_map(|line| line.strip_prefix("security create-keychain -p "))
        .and_then(|rest| rest.split_once(' ').map(|(_, path)| path.to_owned()))
        .unwrap_or_else(|| panic!("no keychain created: {log}"));
    assert!(
        log.contains(&format!("security delete-keychain {created}\n")),
        "{log}"
    );
    assert!(log.contains("openssl pkcs12 -in "), "{log}");
    assert!(log.contains("-passin file:"), "{log}");
    fixture.assert_clean(&ran);
}

type Case<'a> = (Vec<&'a str>, Vec<(&'a str, &'a str)>, String);

#[test]
fn bad_or_missing_inputs_fail_before_signing() {
    let fixture = Fixture::new("apple-export-args");
    let usage = "usage: bazel run <export> -- --certificate <identity.p12> --password-file <file> --out <App.ipa>";
    let ws = fixture.path("ws");
    let cases: Vec<Case> = vec![
        (
            ARGS[2..].to_vec(),
            vec![],
            format!("--certificate names the signing identity .p12 file; {usage}"),
        ),
        (
            [&ARGS[..2], &ARGS[4..]].concat(),
            vec![],
            format!("--password-file names the file holding the .p12 password; {usage}"),
        ),
        (
            ARGS[..4].to_vec(),
            vec![],
            format!("--out names the .ipa to write; {usage}"),
        ),
        (
            [&ARGS[..], &["hunter2"][..]].concat(),
            vec![],
            format!("unexpected argument; {usage}"),
        ),
        (
            vec![
                "--certificate=missing.p12",
                "--password-file=password.txt",
                "--out=dist/Ledger.ipa",
            ],
            vec![],
            format!("--certificate {ws}/missing.p12 is not a file"),
        ),
        (
            vec![
                "--certificate",
                "identity.p12",
                "--password-file",
                "nope.txt",
                "--out",
                "dist/Ledger.ipa",
            ],
            vec![],
            format!("--password-file {ws}/nope.txt is not a file"),
        ),
        (
            vec![
                "--certificate",
                "identity.p12",
                "--password-file",
                "password.txt",
                "--out",
                "dist/Ledger.zip",
            ],
            vec![],
            format!("--out {ws}/dist/Ledger.zip must name an .ipa file"),
        ),
        (
            vec![
                "--certificate",
                "identity.p12",
                "--password-file",
                "password.txt",
                "--out",
                "gone/Ledger.ipa",
            ],
            vec![],
            format!("--out directory {ws}/gone does not exist"),
        ),
        (
            ARGS.to_vec(),
            vec![("DX_EXPORT_META", "")],
            "DX_EXPORT_META names the export metadata of one device app".to_owned(),
        ),
    ];
    for (args, envs, want) in cases {
        let ran = fixture.export(&args, &envs);
        fails(&ran, 2, &want);
        fixture.assert_clean(&ran);
    }
    assert!(!fixture.log().contains("codesign"), "{}", fixture.log());
}

#[test]
fn out_inside_the_output_base_fails() {
    let fixture = Fixture::new("apple-export-outbase");
    let real = fixture.path("ob/execroot/_main/bazel-out/darwin_arm64-fastbuild/bin");
    for out in [
        "bazel-out/darwin_arm64-fastbuild/bin/Ledger.ipa".to_owned(),
        fixture.path("ob/Ledger.ipa"),
    ] {
        let mut args = ARGS[..4].to_vec();
        args.extend_from_slice(&["--out", out.as_str()]);
        let ran = fixture.export(&args, &[]);
        let resolved = if out.starts_with('/') {
            out.clone()
        } else {
            format!("{real}/Ledger.ipa")
        };
        fails(
            &ran,
            2,
            &format!(
                "--out {resolved} is inside the Bazel output base; write the IPA outside bazel-out"
            ),
        );
    }
}

#[test]
fn a_profile_that_does_not_cover_the_app_fails() {
    let expired = PROFILE.replace("2030-01-01", "2026-01-01");
    let other_app = PROFILE.replace(
        "ABCDE12345.com.example.ledger",
        "ABCDE12345.com.example.other",
    );
    let other_profile = PROFILE.replace("Ledger Development", "Other Development");
    let other_cert = PROFILE.replace("ab:cd:ef:01", "99:88:77:66");
    let narrow = PROFILE.replace("Entitlements.get-task-allow=true\n", "");
    let cases = [
        (&expired, &expired, "provisioning profile {p} expired on 2026-01-01T00:00:00Z"),
        (
            &other_app,
            &other_app,
            "provisioning profile {p} covers ABCDE12345.com.example.other, not ABCDE12345.com.example.ledger",
        ),
        (
            &other_profile,
            &PROFILE.to_owned(),
            "{ipa} embeds a provisioning profile other than the declared {p}",
        ),
        (&other_cert, &other_cert, "certificate ABCDEF01 is not in the provisioning profile {p}"),
        (
            &narrow,
            &narrow,
            "entitlement 'get-task-allow' is not allowed by the provisioning profile {p}",
        ),
    ];
    for (declared, embedded, want) in cases {
        let fixture = Fixture::with_profile("apple-export-profile", declared, embedded);
        let ran = fixture.export(&ARGS, &[]);
        let want = want
            .replace("{p}", &fixture.path("dev.mobileprovision"))
            .replace("{ipa}", &fixture.path("Ledger.ipa"));
        fails(&ran, 2, &want);
        assert!(
            !fixture.log().contains("codesign --force"),
            "{}",
            fixture.log()
        );
        assert!(!fixture.root.join("ws/dist/Ledger.ipa").exists());
        fixture.assert_clean(&ran);
    }
}

#[test]
fn a_wildcard_profile_covers_the_bundle_id() {
    let profile = PROFILE.replace("ABCDE12345.com.example.ledger", "ABCDE12345.*");
    let fixture = Fixture::with_profile("apple-export-wildcard", &profile, &profile);
    let ran = fixture.export(&ARGS, &[]);
    assert_eq!(ran.status.code(), Some(0), "{}", ran.combined());
}

#[test]
fn a_wrong_password_fails_without_a_keychain() {
    let fixture = Fixture::new("apple-export-password");
    let ran = fixture.export(&ARGS, &[("FAKE_PASSWORD", "other")]);
    fails(
        &ran,
        2,
        &format!(
            "cannot read {} with the password in --password-file",
            fixture.path("ws/identity.p12")
        ),
    );
    assert!(
        !fixture.log().contains("create-keychain"),
        "{}",
        fixture.log()
    );
    fixture.assert_clean(&ran);
}

#[test]
fn a_signed_app_that_fails_verification_is_not_written() {
    let cases = [
        (
            ("FAKE_ARCHS", "arm64 x86_64"),
            "Ledger carries arm64 x86_64, want arm64".to_owned(),
        ),
        (
            ("FAKE_PLATFORM", "7"),
            "Ledger is built for platform '7', want 2 (iphoneos)".to_owned(),
        ),
        (
            ("FAKE_DROP", "Assets.car"),
            "signing changed the bundle contents at Payload/Ledger.app/Assets.car".to_owned(),
        ),
        (
            ("FAKE_VERIFY_EXIT", "3"),
            "signed Ledger.app fails codesign --verify --deep --strict".to_owned(),
        ),
        (
            ("FAKE_SIGNED_TEAM", "ZZZZZ99999"),
            "signed Ledger.app carries TeamIdentifier 'ZZZZZ99999', want ABCDE12345".to_owned(),
        ),
    ];
    for (env, want) in cases {
        let fixture = Fixture::new("apple-export-verify");
        let ran = fixture.export(&ARGS, &[env]);
        fails(&ran, 1, &want);
        assert!(!fixture.root.join("ws/dist/Ledger.ipa").exists());
        assert!(!fixture.root.join("ws/dist/Ledger.ipa.txt").exists());
        assert!(
            fixture.log().contains("security delete-keychain "),
            "{}",
            fixture.log()
        );
        fixture.assert_clean(&ran);
    }
}

#[test]
fn codesign_failures_keep_their_exit_code_and_clean_up() {
    let fixture = Fixture::new("apple-export-codesign");
    let ran = fixture.export(&ARGS, &[("FAKE_SIGN_EXIT", "41")]);
    assert_eq!(ran.status.code(), Some(41), "{}", ran.combined());
    assert_eq!(
        ran.stderr,
        "codesign: errSecInternalComponent\n",
        "{}",
        ran.combined()
    );
    assert!(
        fixture.log().contains("security delete-keychain "),
        "{}",
        fixture.log()
    );
    assert!(!fixture.root.join("ws/dist/Ledger.ipa").exists());
    fixture.assert_clean(&ran);
}

#[test]
fn a_host_without_the_signing_tools_fails_instead_of_running_anything() {
    for (var, name) in [
        ("XCRUN", "xcrun"),
        ("SECURITY", "security"),
        ("CODESIGN", "codesign"),
        ("OPENSSL", "openssl"),
    ] {
        let fixture = Fixture::new("apple-export-tools");
        let missing = format!("/nonexistent/{name}");
        let ran = fixture.export(&ARGS, &[(var, missing.as_str())]);
        fails(
            &ran,
            127,
            &format!(
                "no {name} executable {missing} on PATH; export needs a macOS host with Xcode"
            ),
        );
        assert_eq!(fixture.log(), "");
    }
}
