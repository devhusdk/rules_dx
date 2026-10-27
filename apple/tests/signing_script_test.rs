use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use dx_testing::{mkscratch, resolve_runfiles, run, Run};

const FAKE_SECURITY: &str = r#"#!/bin/sh
printf 'security %s\n' "$*" >>"$FAKE_LOG"
case "$1" in
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
        printf '  1) %s "Developer ID Application: Ledger (ABCDE12345)"\n' "$FAKE_IDENTITY"
        if [ -n "${FAKE_OTHER_IDENTITY:-}" ]; then
            printf '  2) %s "Developer ID Application: Ledger Labs (FGHIJ67890)"\n' "$FAKE_OTHER_IDENTITY"
        fi
        printf '     2 valid identities found\n'
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
esac
"#;

const FAKE_PLUTIL: &str = r#"#!/bin/sh
printf 'plutil %s\n' "$*" >>"$FAKE_LOG"
case "$1" in
    -convert)
        cp "$5" "$4"
        ;;
    -extract)
        if line=$(grep -m 1 "^$2=" "$6"); then
            printf '%s\n' "${line#*=}"
        else
            found=$(sed -n "s/.*\"$2\" *: *\"\([^\"]*\)\".*/\1/p" "$6" | head -n 1)
            [ -n "$found" ] || exit 1
            printf '%s\n' "$found"
        fi
        ;;
esac
"#;

const FAKE_CODESIGN: &str = r#"#!/bin/sh
printf 'codesign %s\n' "$*" >>"$FAKE_LOG"
for last; do :; done
case "$1" in
    -d*)
        if [ "${2:-}" = --entitlements ]; then
            case "$last" in
                */Share.appex) printf '%s' "${FAKE_NESTED_ENTITLEMENTS:-}" ;;
            esac
            exit 0
        fi
        printf 'Executable=%s/Contents/MacOS/Ledger\n' "$last" >&2
        printf 'CodeDirectory v=20500 size=512 flags=%s hashes=8+7 location=embedded\n' "${FAKE_FLAGS:-0x10000(runtime)}" >&2
        if [ -n "${FAKE_ADHOC:-}" ]; then
            echo "Signature=adhoc" >&2
        else
            echo "Authority=Developer ID Application: Ledger (ABCDE12345)" >&2
            [ -n "${FAKE_NO_TIMESTAMP:-}" ] || echo "Timestamp=Oct 27, 2026 at 10:00:00" >&2
        fi
        echo "TeamIdentifier=ABCDE12345" >&2
        ;;
    --verify)
        exit "${FAKE_VERIFY_EXIT:-0}"
        ;;
    --force)
        if [ "${FAKE_SIGN_EXIT:-0}" != 0 ]; then
            echo "${FAKE_SIGN_STDERR:-codesign: errSecInternalComponent}" >&2
            exit "$FAKE_SIGN_EXIT"
        fi
        if [ -d "$last/Contents" ]; then
            mkdir -p "$last/Contents/_CodeSignature"
            echo signed >"$last/Contents/_CodeSignature/CodeResources"
        elif [ -d "$last" ]; then
            mkdir -p "$last/_CodeSignature"
            echo signed >"$last/_CodeSignature/CodeResources"
        fi
        ;;
esac
"#;

const FAKE_DITTO: &str = r#"#!/bin/sh
printf 'ditto %s\n' "$*" >>"$FAKE_LOG"
case "$1" in
    -x)
        mkdir -p "$4"
        unzip -q "$3" -d "$4"
        ;;
    -c)
        cd "$(dirname "$4")" && zip -qry "$5" "$(basename "$4")"
        ;;
    *)
        cp -R "$1" "$2"
        ;;
esac
"#;

const FAKE_XCRUN: &str = r#"#!/bin/sh
printf 'xcrun %s\n' "$*" >>"$FAKE_LOG"
case "$1 $2" in
    "lipo -archs")
        printf '%s\n' "${FAKE_ARCHS:-arm64 x86_64}"
        ;;
    "notarytool submit")
        if [ -n "${FAKE_OFFLINE:-}" ]; then
            echo 'Error: NSURLErrorDomain Code=-1009 "The Internet connection appears to be offline."' >&2
            exit 69
        fi
        printf '{"id":"2efe2717-52ef-43a5-96dc-0797e4ca1041","status":"%s","message":"Processing complete"}\n' "${FAKE_STATUS:-Accepted}"
        exit "${FAKE_SUBMIT_EXIT:-0}"
        ;;
    "notarytool log")
        printf '{"status":"%s","issues":null}\n' "${FAKE_STATUS:-Accepted}" >"$4"
        ;;
    "stapler staple")
        exit "${FAKE_STAPLE_EXIT:-0}"
        ;;
    "stapler validate")
        exit "${FAKE_VALIDATE_EXIT:-0}"
        ;;
esac
"#;

const FAKE_SPCTL: &str = r#"#!/bin/sh
printf 'spctl %s\n' "$*" >>"$FAKE_LOG"
echo "$3: accepted" >&2
exit "${FAKE_SPCTL_EXIT:-0}"
"#;

const ENTITLEMENTS: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0">
<dict>
	<key>com.apple.security.app-sandbox</key>
	<true/>
	<key>com.apple.security.network.client</key>
	<true/>
</dict>
</plist>
"#;

const DEBUGGABLE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0">
<dict>
	<key>com.apple.security.app-sandbox</key>
	<true/>
	<key>com.apple.security.get-task-allow</key>
	<true/>
</dict>
</plist>
"#;

const PASSWORD: &str = "hunter2-p12-secret";

const P8: &str = "-----BEGIN PRIVATE KEY-----\nnotary-p8-secret\n-----END PRIVATE KEY-----\n";

const KEYCHAINS: &str = "    \"/Users/dev/Library/Keychains/login.keychain-db\"\n    \"/Library/Keychains/System.keychain\"\n";

const INFO: &str = "CFBundleExecutable=Ledger\nCFBundleIdentifier=com.example.ledger\n";

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

fn write_bytes(path: &Path, body: &[u8]) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap_or_else(|error| panic!("mkdir: {error}"));
    }
    std::fs::write(path, body).unwrap_or_else(|error| panic!("write {}: {error}", path.display()));
}

const MACHO: [u8; 8] = [0xcf, 0xfa, 0xed, 0xfe, 0x0c, 0x00, 0x00, 0x01];

const NESTED_DEBUGGABLE: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?><plist version=\"1.0\"><dict><key>com.apple.security.get-task-allow</key><true/></dict></plist>";

fn nested_bundle(bundle: &Path, executable: &str) {
    write(
        &bundle.join("Contents/Info.plist"),
        &format!("CFBundleExecutable={executable}\n"),
    );
    write_bytes(&bundle.join("Contents/MacOS").join(executable), &MACHO);
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

fn app_tree(app: &Path) {
    write(&app.join("Contents/Info.plist"), INFO);
    write(&app.join("Contents/MacOS/Ledger"), "binary");
    write(&app.join("Contents/Resources/Assets.car"), "assets");
    write(&app.join("Contents/_CodeSignature/CodeResources"), "adhoc");
    write(
        &app.join("Contents/Frameworks/Share.framework/Versions/A/Share"),
        "framework",
    );
    write(&app.join("Contents/Frameworks/libz.dylib"), "dylib");
    std::os::unix::fs::symlink("libz.dylib", app.join("Contents/Frameworks/libz.1.dylib"))
        .unwrap_or_else(|error| panic!("symlink: {error}"));
}

fn files_under(root: &Path) -> Vec<String> {
    let mut found = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap_or_else(|error| panic!("read_dir: {error}")) {
            let path = entry
                .unwrap_or_else(|error| panic!("entry: {error}"))
                .path();
            if path.is_dir() && !path.is_symlink() {
                stack.push(path);
            } else {
                found.push(text(&path));
            }
        }
    }
    found
}

impl Fixture {
    fn new(prefix: &str) -> Fixture {
        Fixture::with(prefix, false, Some(ENTITLEMENTS))
    }

    fn with(prefix: &str, tree: bool, entitlements: Option<&str>) -> Fixture {
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
            ("DITTO".to_owned(), tool(&root, "ditto", FAKE_DITTO)),
            ("XCRUN".to_owned(), tool(&root, "xcrun", FAKE_XCRUN)),
            ("SPCTL".to_owned(), tool(&root, "spctl", FAKE_SPCTL)),
        ];
        let archive = if tree {
            let app = root.join("bin-out/Ledger.app");
            app_tree(&app);
            app
        } else {
            app_tree(&root.join("payload/Ledger.app"));
            let zipped = std::process::Command::new("zip")
                .current_dir(root.join("payload"))
                .args(["-qry", "../Ledger.zip", "Ledger.app"])
                .status()
                .unwrap_or_else(|error| panic!("zip: {error}"));
            assert!(zipped.success());
            root.join("Ledger.zip")
        };
        let entitlements_path = match entitlements {
            Some(body) => {
                write(&root.join("Ledger.entitlements"), body);
                root.join("Ledger.entitlements").display().to_string()
            }
            None => String::new(),
        };
        write(&root.join("keychains.txt"), KEYCHAINS);
        write(&root.join("ws/identity.p12"), "p12");
        write(&root.join("ws/password.txt"), PASSWORD);
        write(&root.join("ws/AuthKey.p8"), P8);
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
            "bundle_id=com.example.ledger\narchive={}\nentitlements={entitlements_path}\n",
            archive.display(),
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

    fn launch(&self, var: &str, args: &[&str], envs: &[(&str, &str)]) -> Run {
        let log = self.path("log.txt");
        let keychains = self.path("keychains.txt");
        let meta = self.path("meta.txt");
        let ws = self.path("ws");
        let tmp = self.path("tmp");
        let mut all: Vec<(&str, &str)> = self
            .tools
            .iter()
            .map(|(key, value)| (key.as_str(), value.as_str()))
            .collect();
        all.extend_from_slice(&[
            ("FAKE_LOG", log.as_str()),
            ("FAKE_KEYCHAINS", keychains.as_str()),
            ("FAKE_PASSWORD", PASSWORD),
            ("FAKE_CERT", "AB:CD:EF:01"),
            ("FAKE_IDENTITY", "ABCDEF01"),
            ("DX_SIGN_META", meta.as_str()),
            ("BUILD_WORKSPACE_DIRECTORY", ws.as_str()),
            ("BUILD_WORKING_DIRECTORY", ws.as_str()),
            ("TMPDIR", tmp.as_str()),
        ]);
        all.extend_from_slice(envs);
        let script = resolve_runfiles(
            &std::env::var(var).unwrap_or_else(|_| panic!("{var} must name a script")),
        );
        run(&script, args, &all).unwrap_or_else(|error| panic!("run: {error}"))
    }

    fn sign(&self, args: &[&str], envs: &[(&str, &str)]) -> Run {
        self.launch("DX_CODESIGN_APP", args, envs)
    }

    fn notarize(&self, args: &[&str], envs: &[(&str, &str)]) -> Run {
        self.launch("DX_NOTARIZE_APP", args, envs)
    }

    fn signed_app(&self) {
        app_tree(&self.root.join("ws/dist/Ledger.app"));
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
        let log = self.log();
        let mut haystacks = vec![ran.stdout.as_str(), ran.stderr.as_str(), log.as_str()];
        let written = files_under(&self.root.join("ws/dist"));
        haystacks.extend(written.iter().map(String::as_str));
        for haystack in haystacks {
            assert!(
                !haystack.contains(PASSWORD),
                "the password never leaks: {haystack}"
            );
            assert!(
                !haystack.contains("notary-p8-secret"),
                "the notary key never leaks: {haystack}"
            );
        }
    }
}

const SIGN: [&str; 6] = [
    "--certificate",
    "identity.p12",
    "--password-file",
    "password.txt",
    "--out",
    "dist/Ledger.app",
];

const KEY: [&str; 8] = [
    "--app",
    "dist/Ledger.app",
    "--key",
    "AuthKey.p8",
    "--key-id",
    "2X9R4HXF34",
    "--issuer",
    "69a6de7f-0000-47e3-e053-5b8c7c11a4d1",
];

fn fails(ran: &Run, me: &str, code: i32, stderr: &str) {
    assert_eq!(ran.status.code(), Some(code), "{}", ran.combined());
    assert_eq!(
        ran.stderr,
        format!("{me}: {stderr}\n"),
        "{}",
        ran.combined()
    );
}

fn signs(log: &str) -> Vec<&str> {
    log.lines()
        .filter(|line| line.starts_with("codesign --force"))
        .collect()
}

const REPORT: &str = "bundle_id=com.example.ledger
team=ABCDE12345
authority=Developer ID Application: Ledger (ABCDE12345)
certificate_sha1=ABCDEF01
timestamp=Oct 27, 2026 at 10:00:00
entitlements=com.apple.security.app-sandbox,com.apple.security.network.client
archs=arm64 x86_64
runtime=yes
";

#[test]
fn sign_signs_frameworks_then_the_app_with_runtime_and_timestamp() {
    for tree in [false, true] {
        let fixture = Fixture::with("apple-sign-ok", tree, Some(ENTITLEMENTS));
        let ran = fixture.sign(&SIGN, &[]);
        assert_eq!(ran.status.code(), Some(0), "{}", ran.combined());
        let out = fixture.root.join("ws/dist/Ledger.app");
        assert!(
            out.join("Contents/Resources/Assets.car").is_file(),
            "{}",
            ran.combined()
        );
        assert_eq!(
            text(&out.join("Contents/_CodeSignature/CodeResources")),
            "signed\n"
        );
        assert_eq!(text(&fixture.root.join("ws/dist/Ledger.app.txt")), REPORT);
        let log = fixture.log();
        let signed = signs(&log);
        assert_eq!(signed.len(), 3, "{log}");
        let mut nested: Vec<&str> = signed[..2]
            .iter()
            .map(|line| line.rsplit('/').next().unwrap_or_default())
            .collect();
        nested.sort_unstable();
        assert_eq!(nested, ["Share.framework", "libz.dylib"], "{log}");
        assert!(signed[2].ends_with("/Ledger.app"), "{log}");
        for (index, line) in signed.iter().enumerate() {
            assert!(
                line.starts_with("codesign --force --options runtime --timestamp --keychain "),
                "{log}"
            );
            assert!(line.contains(" --sign ABCDEF01 "), "{log}");
            assert!(!line.contains("--deep"), "{log}");
            assert_eq!(line.contains("--entitlements "), index == 2, "{log}");
        }
        assert!(log.contains("codesign --verify --deep --strict "), "{log}");
        let created = log
            .lines()
            .find_map(|line| line.strip_prefix("security create-keychain -p "))
            .and_then(|rest| rest.split_once(' ').map(|(_, path)| path.to_owned()))
            .unwrap_or_else(|| panic!("no keychain created: {log}"));
        assert!(
            log.contains(&format!("security delete-keychain {created}\n")),
            "{log}"
        );
        assert!(log.contains("-passin file:"), "{log}");
        fixture.assert_clean(&ran);
    }
}

#[test]
fn sign_without_entitlements_reports_none() {
    let fixture = Fixture::with("apple-sign-plain", false, None);
    let ran = fixture.sign(&SIGN, &[]);
    assert_eq!(ran.status.code(), Some(0), "{}", ran.combined());
    assert!(
        !fixture.log().contains("--entitlements"),
        "{}",
        fixture.log()
    );
    assert!(text(&fixture.root.join("ws/dist/Ledger.app.txt")).contains("\nentitlements=\n"));
    fixture.assert_clean(&ran);
}

type Case<'a> = (Vec<&'a str>, Vec<(&'a str, &'a str)>, String);

#[test]
fn sign_rejects_bad_input_before_signing() {
    let fixture = Fixture::new("apple-sign-args");
    let usage = "usage: bazel run <sign> -- (--certificate <identity.p12> --password-file <file> | --identity <SHA-1 or name> [--keychain <keychain>]) --out <App.app>";
    let ws = fixture.path("ws");
    let cases: Vec<Case> = vec![
        (
            SIGN[2..].to_vec(),
            vec![],
            format!("--certificate names the Developer ID identity .p12 file, or --identity names one in a keychain; {usage}"),
        ),
        (
            [&SIGN[..2], &SIGN[4..]].concat(),
            vec![],
            format!("--password-file names the file holding the .p12 password; {usage}"),
        ),
        (
            SIGN[..4].to_vec(),
            vec![],
            format!("--out names the .app to write; {usage}"),
        ),
        (
            [&SIGN[..], &["hunter2"][..]].concat(),
            vec![],
            format!("unexpected argument; {usage}"),
        ),
        (
            vec![
                "--certificate=missing.p12",
                "--password-file=password.txt",
                "--out=dist/Ledger.app",
            ],
            vec![],
            format!("--certificate {ws}/missing.p12 is not a file"),
        ),
        (
            [&SIGN[..4], &["--out", "dist/Ledger.zip"][..]].concat(),
            vec![],
            format!("--out {ws}/dist/Ledger.zip must name an .app bundle"),
        ),
        (
            [&SIGN[..4], &["--out", "gone/Ledger.app"][..]].concat(),
            vec![],
            format!("--out directory {ws}/gone does not exist"),
        ),
        (
            [&SIGN[..], &["--identity", "ABCDEF01"][..]].concat(),
            vec![],
            format!("pass --identity or --certificate with --password-file, not both; {usage}"),
        ),
        (
            [&SIGN[4..], &["--keychain", "ci.keychain-db"][..]].concat(),
            vec![],
            format!("--keychain only goes with --identity; {usage}"),
        ),
        (
            [&SIGN[4..], &["--identity", "ABCDEF01", "--keychain", "keys/gone.keychain-db"][..]]
                .concat(),
            vec![],
            format!("--keychain {ws}/keys/gone.keychain-db is not a file"),
        ),
        (
            SIGN.to_vec(),
            vec![("DX_SIGN_META", "")],
            "DX_SIGN_META names the signing metadata of one macOS app".to_owned(),
        ),
        (
            SIGN.to_vec(),
            vec![("FAKE_PASSWORD", "other")],
            format!("cannot read {ws}/identity.p12 with the password in --password-file"),
        ),
    ];
    for (args, envs, want) in cases {
        let ran = fixture.sign(&args, &envs);
        fails(&ran, "dx-apple-sign", 2, &want);
        assert!(!fixture.root.join("ws/dist/Ledger.app").exists());
        fixture.assert_clean(&ran);
    }
    assert!(!fixture.log().contains("codesign"), "{}", fixture.log());
    assert!(
        !fixture.log().contains("create-keychain"),
        "{}",
        fixture.log()
    );
}

#[test]
fn sign_out_inside_the_output_base_fails() {
    let fixture = Fixture::new("apple-sign-outbase");
    let real = fixture.path("ob/execroot/_main/bazel-out/darwin_arm64-fastbuild/bin");
    for out in [
        "bazel-out/darwin_arm64-fastbuild/bin/Ledger.app".to_owned(),
        fixture.path("ob/Ledger.app"),
    ] {
        let mut args = SIGN[..4].to_vec();
        args.extend_from_slice(&["--out", out.as_str()]);
        let ran = fixture.sign(&args, &[]);
        let resolved = if out.starts_with('/') {
            out.clone()
        } else {
            format!("{real}/Ledger.app")
        };
        fails(
            &ran,
            "dx-apple-sign",
            2,
            &format!(
                "--out {resolved} is inside the Bazel output base; write the app outside bazel-out"
            ),
        );
    }
}

#[test]
fn sign_rejects_debuggable_apps_and_nested_code() {
    let debuggable = Fixture::with("apple-sign-debug", false, Some(DEBUGGABLE));
    let ran = debuggable.sign(&SIGN, &[]);
    fails(
        &ran,
        "dx-apple-sign",
        2,
        "entitlement 'com.apple.security.get-task-allow' is true; notarization rejects debuggable apps",
    );
    assert!(signs(&debuggable.log()).is_empty(), "{}", debuggable.log());
    debuggable.assert_clean(&ran);

    for (nested, body, want) in [
        (
            "Contents/Frameworks/Share.framework/Versions/A/XPCServices/Fetch.xpc/Contents/Info.plist",
            &b"x"[..],
            "Ledger.app carries nested code at Contents/Frameworks/Share.framework/Versions/A/XPCServices; signing code nested in a framework is not supported yet",
        ),
        (
            "Contents/Resources/tool",
            &MACHO[..],
            "Ledger.app carries code at Contents/Resources/tool outside the places this launcher signs; move it into Contents/Frameworks, PlugIns, XPCServices, Helpers, MacOS, or Library/LoginItems, SystemExtensions, QuickLook or Spotlight",
        ),
        (
            "Contents/PlugIns/Share.appex/Contents/Resources/tool",
            &MACHO[..],
            "Ledger.app carries code at Contents/PlugIns/Share.appex/Contents/Resources/tool outside the places this launcher signs; move it into Contents/Frameworks, PlugIns, XPCServices, Helpers, MacOS, or Library/LoginItems, SystemExtensions, QuickLook or Spotlight",
        ),
        (
            "Contents/XPCServices/Fetch.xpc/Contents/MacOS/Fetch",
            &MACHO[..],
            "Ledger.app carries Contents/XPCServices/Fetch.xpc without Contents/Info.plist; cannot sign it",
        ),
    ] {
        let fixture = Fixture::with("apple-sign-nested", true, Some(ENTITLEMENTS));
        let app = fixture.root.join("bin-out/Ledger.app");
        if nested.starts_with("Contents/PlugIns/") {
            nested_bundle(&app.join("Contents/PlugIns/Share.appex"), "Share");
        }
        write_bytes(&app.join(nested), body);
        let ran = fixture.sign(&SIGN, &[]);
        fails(&ran, "dx-apple-sign", 2, want);
        assert!(signs(&fixture.log()).is_empty(), "{}", fixture.log());
        assert!(!fixture.root.join("ws/dist/Ledger.app").exists());
        fixture.assert_clean(&ran);
    }

    let fixture = Fixture::with("apple-sign-nested-debug", true, Some(ENTITLEMENTS));
    nested_bundle(
        &fixture
            .root
            .join("bin-out/Ledger.app/Contents/PlugIns/Share.appex"),
        "Share",
    );
    let ran = fixture.sign(&SIGN, &[("FAKE_NESTED_ENTITLEMENTS", NESTED_DEBUGGABLE)]);
    fails(
        &ran,
        "dx-apple-sign",
        2,
        "Contents/PlugIns/Share.appex sets entitlement 'com.apple.security.get-task-allow'; notarization rejects debuggable code",
    );
    assert!(signs(&fixture.log()).is_empty(), "{}", fixture.log());
    fixture.assert_clean(&ran);
}

#[test]
fn sign_signs_nested_code_inside_out_and_keeps_its_entitlements() {
    let fixture = Fixture::with("apple-sign-inside-out", true, Some(ENTITLEMENTS));
    let app = fixture.root.join("bin-out/Ledger.app");
    nested_bundle(&app.join("Contents/PlugIns/Share.appex"), "Share");
    nested_bundle(&app.join("Contents/XPCServices/Fetch.xpc"), "Fetch");
    let login = app.join("Contents/Library/LoginItems/Agent.app");
    nested_bundle(&login, "Agent");
    write_bytes(
        &login.join("Contents/Frameworks/Core.framework/Versions/A/Core"),
        &MACHO,
    );
    write_bytes(&app.join("Contents/Helpers/ledgerctl"), &MACHO);
    write_bytes(&app.join("Contents/MacOS/ledger-cli"), &MACHO);
    write(&app.join("Contents/MacOS/README"), "not code");
    let ran = fixture.sign(&SIGN, &[("FAKE_NESTED_ENTITLEMENTS", ENTITLEMENTS)]);
    assert_eq!(ran.status.code(), Some(0), "{}", ran.combined());
    let log = fixture.log();
    let signed: Vec<String> = signs(&log)
        .iter()
        .map(|line| {
            let path = line.rsplit(' ').next().unwrap_or_default();
            path.split_once("/Ledger.app")
                .map(|(_, rest)| format!("Ledger.app{rest}"))
                .unwrap_or_default()
        })
        .collect();
    let position = |want: &str| {
        signed
            .iter()
            .position(|path| path == want)
            .unwrap_or_else(|| panic!("{want} is not signed: {log}"))
    };
    let core = position(
        "Ledger.app/Contents/Library/LoginItems/Agent.app/Contents/Frameworks/Core.framework",
    );
    let agent = position("Ledger.app/Contents/Library/LoginItems/Agent.app");
    let main = position("Ledger.app");
    assert!(core < agent, "{log}");
    for nested in [
        "Ledger.app/Contents/Frameworks/Share.framework",
        "Ledger.app/Contents/Frameworks/libz.dylib",
        "Ledger.app/Contents/PlugIns/Share.appex",
        "Ledger.app/Contents/XPCServices/Fetch.xpc",
        "Ledger.app/Contents/Helpers/ledgerctl",
        "Ledger.app/Contents/MacOS/ledger-cli",
    ] {
        assert!(position(nested) < main, "{log}");
    }
    assert_eq!(main, signed.len() - 1, "{log}");
    assert_eq!(signed.len(), 9, "{log}");
    for line in signs(&log) {
        assert!(!line.contains("--deep"), "{log}");
        let path = line.rsplit(' ').next().unwrap_or_default();
        let bundle_or_tool = [".appex", ".xpc", "/Agent.app", "/ledgerctl", "/ledger-cli"]
            .iter()
            .any(|suffix| path.ends_with(suffix));
        assert_eq!(
            line.contains(" --preserve-metadata=entitlements "),
            bundle_or_tool,
            "{log}"
        );
        assert_eq!(
            line.contains(" --entitlements "),
            path.ends_with("/Ledger.app"),
            "{log}"
        );
    }
    fixture.assert_clean(&ran);
}

#[test]
fn sign_with_a_keychain_identity_skips_the_temporary_keychain() {
    let fixture = Fixture::new("apple-sign-identity");
    write(&fixture.root.join("ws/ci.keychain-db"), "keychain");
    let keychain = fixture.path("ws/ci.keychain-db");
    let ran = fixture.sign(
        &[
            "--identity",
            "abcdef01",
            "--keychain",
            "ci.keychain-db",
            "--out",
            "dist/Ledger.app",
        ],
        &[],
    );
    assert_eq!(ran.status.code(), Some(0), "{}", ran.combined());
    let log = fixture.log();
    assert!(
        log.contains(&format!(
            "security find-identity -v -p codesigning {keychain}\n"
        )),
        "{log}"
    );
    for line in signs(&log) {
        assert!(
            line.starts_with(&format!(
                "codesign --force --options runtime --timestamp --keychain {keychain} --sign ABCDEF01 "
            )),
            "{log}"
        );
    }
    assert!(!log.contains("create-keychain"), "{log}");
    assert!(!log.contains("delete-keychain"), "{log}");
    assert!(!log.contains("openssl"), "{log}");
    assert_eq!(text(&fixture.root.join("ws/dist/Ledger.app.txt")), REPORT);
    fixture.assert_clean(&ran);

    let by_name = Fixture::new("apple-sign-identity-env");
    let ran = by_name.sign(
        &["--out", "dist/Ledger.app"],
        &[(
            "DX_APPLE_SIGN_IDENTITY",
            "Developer ID Application: Ledger (ABCDE12345)",
        )],
    );
    assert_eq!(ran.status.code(), Some(0), "{}", ran.combined());
    let log = by_name.log();
    assert!(
        log.contains("security find-identity -v -p codesigning\n"),
        "{log}"
    );
    for line in signs(&log) {
        assert!(
            line.starts_with("codesign --force --options runtime --timestamp --sign ABCDEF01 "),
            "{log}"
        );
    }
    by_name.assert_clean(&ran);

    for (identity, envs, want) in [
        (
            "Apple Development",
            vec![],
            "no valid code signing identity matches --identity Apple Development",
        ),
        (
            "Developer ID Application: Ledger",
            vec![("FAKE_OTHER_IDENTITY", "0123ABCD")],
            "--identity Developer ID Application: Ledger matches more than one code signing identity; pass its SHA-1",
        ),
    ] {
        let fixture = Fixture::new("apple-sign-identity-bad");
        let ran = fixture.sign(&["--identity", identity, "--out", "dist/Ledger.app"], &envs);
        fails(&ran, "dx-apple-sign", 2, want);
        assert!(signs(&fixture.log()).is_empty(), "{}", fixture.log());
        assert!(!fixture.root.join("ws/dist/Ledger.app").exists());
        fixture.assert_clean(&ran);
    }
}

#[test]
fn sign_verification_failures_write_nothing() {
    for (env, want) in [
        (
            ("FAKE_VERIFY_EXIT", "3"),
            "signed Ledger.app fails codesign --verify --deep --strict",
        ),
        (
            ("FAKE_FLAGS", "0x0(none)"),
            "signed Ledger.app carries no hardened runtime flag",
        ),
        (
            ("FAKE_NO_TIMESTAMP", "1"),
            "signed Ledger.app carries no secure timestamp",
        ),
        (
            ("FAKE_ADHOC", "1"),
            "signed Ledger.app carries no signing authority",
        ),
    ] {
        let fixture = Fixture::new("apple-sign-verify");
        let ran = fixture.sign(&SIGN, &[env]);
        fails(&ran, "dx-apple-sign", 1, want);
        assert!(!fixture.root.join("ws/dist/Ledger.app").exists());
        assert!(!fixture.root.join("ws/dist/Ledger.app.txt").exists());
        assert!(
            fixture.log().contains("security delete-keychain "),
            "{}",
            fixture.log()
        );
        fixture.assert_clean(&ran);
    }
}

#[test]
fn sign_codesign_failures_keep_their_code_or_report_offline() {
    let fixture = Fixture::new("apple-sign-codesign");
    let ran = fixture.sign(&SIGN, &[("FAKE_SIGN_EXIT", "41")]);
    assert_eq!(ran.status.code(), Some(41), "{}", ran.combined());
    assert_eq!(ran.stderr, "codesign: errSecInternalComponent\n");
    assert!(!fixture.root.join("ws/dist/Ledger.app").exists());
    fixture.assert_clean(&ran);

    let offline = Fixture::new("apple-sign-offline");
    let ran = offline.sign(
        &SIGN,
        &[
            ("FAKE_SIGN_EXIT", "1"),
            (
                "FAKE_SIGN_STDERR",
                "Ledger.app: The timestamp service is not available.",
            ),
        ],
    );
    assert_eq!(ran.status.code(), Some(3), "{}", ran.combined());
    assert_eq!(
        ran.stderr,
        "Ledger.app: The timestamp service is not available.\ndx-apple-sign: the Apple timestamp service is not reachable; retry when online\n"
    );
    assert!(
        offline.log().contains("security delete-keychain "),
        "{}",
        offline.log()
    );
    offline.assert_clean(&ran);
}

#[test]
fn launchers_without_their_tools_fail_before_running_anything() {
    for (var, name) in [
        ("XCRUN", "xcrun"),
        ("SECURITY", "security"),
        ("CODESIGN", "codesign"),
        ("OPENSSL", "openssl"),
        ("PLUTIL", "plutil"),
        ("DITTO", "ditto"),
    ] {
        let fixture = Fixture::new("apple-sign-tools");
        let missing = format!("/nonexistent/{name}");
        let ran = fixture.sign(&SIGN, &[(var, missing.as_str())]);
        fails(
            &ran,
            "dx-apple-sign",
            127,
            &format!(
                "no {name} executable {missing} on PATH; signing needs a macOS host with Xcode"
            ),
        );
        assert_eq!(fixture.log(), "");
    }
    for (var, name) in [("XCRUN", "xcrun"), ("SPCTL", "spctl")] {
        let fixture = Fixture::new("apple-notarize-tools");
        let missing = format!("/nonexistent/{name}");
        let ran = fixture.notarize(&KEY, &[(var, missing.as_str())]);
        fails(
            &ran,
            "dx-apple-notarize",
            127,
            &format!(
                "no {name} executable {missing} on PATH; notarization needs a macOS host with Xcode"
            ),
        );
        assert_eq!(fixture.log(), "");
    }
}

const SUBMISSION: &str = "2efe2717-52ef-43a5-96dc-0797e4ca1041";

#[test]
fn notarize_accepted_staples_and_keeps_the_log() {
    let fixture = Fixture::new("apple-notarize-ok");
    fixture.signed_app();
    let ran = fixture.notarize(&KEY, &[]);
    assert_eq!(ran.status.code(), Some(0), "{}", ran.combined());
    let app = fixture.path("ws/dist/Ledger.app");
    assert_eq!(
        ran.stdout,
        format!("dx-apple-notarize: notarized and stapled {app} (submission {SUBMISSION}); notary log at {app}.notary.json\n")
    );
    assert!(text(&fixture.root.join("ws/dist/Ledger.app.notary.json")).contains("Accepted"));
    let log = fixture.log();
    let key = fixture.path("ws/AuthKey.p8");
    let creds =
        format!("--key {key} --key-id 2X9R4HXF34 --issuer 69a6de7f-0000-47e3-e053-5b8c7c11a4d1");
    let submit = log
        .lines()
        .find(|line| line.starts_with("xcrun notarytool submit "))
        .unwrap_or_else(|| panic!("no submit: {log}"));
    assert!(submit.contains(&creds), "{log}");
    assert!(
        submit.ends_with("--wait --timeout 2h --output-format json"),
        "{log}"
    );
    assert!(
        log.contains(&format!(
            "xcrun notarytool log {SUBMISSION} {app}.notary.json {creds}\n"
        )),
        "{log}"
    );
    assert!(
        log.contains(&format!("ditto -c -k --keepParent {app} ")),
        "{log}"
    );
    assert!(
        log.contains(&format!("xcrun stapler staple {app}\n")),
        "{log}"
    );
    assert!(
        log.contains(&format!("xcrun stapler validate {app}\n")),
        "{log}"
    );
    assert!(
        log.contains(&format!("spctl --assess --type execute -vv {app}\n")),
        "{log}"
    );
    fixture.assert_clean(&ran);
}

#[test]
fn notarize_with_a_keychain_profile_passes_only_the_profile() {
    let fixture = Fixture::new("apple-notarize-profile");
    fixture.signed_app();
    let ran = fixture.notarize(
        &[
            "--app=dist/Ledger.app",
            "--keychain-profile=ledger-notary",
            "--timeout=30m",
        ],
        &[],
    );
    assert_eq!(ran.status.code(), Some(0), "{}", ran.combined());
    let log = fixture.log();
    assert!(
        log.contains(
            " --keychain-profile ledger-notary --wait --timeout 30m --output-format json\n"
        ),
        "{log}"
    );
    assert!(!log.contains("--key "), "{log}");
}

#[test]
fn notarize_invalid_keeps_the_log_and_does_not_staple() {
    let fixture = Fixture::new("apple-notarize-invalid");
    fixture.signed_app();
    let ran = fixture.notarize(
        &KEY,
        &[("FAKE_STATUS", "Invalid"), ("FAKE_SUBMIT_EXIT", "1")],
    );
    let app = fixture.path("ws/dist/Ledger.app");
    fails(
        &ran,
        "dx-apple-notarize",
        1,
        &format!("submission {SUBMISSION} of Ledger.app is Invalid; see the notary log {app}.notary.json"),
    );
    assert!(text(&fixture.root.join("ws/dist/Ledger.app.notary.json")).contains("Invalid"));
    assert!(!fixture.log().contains("stapler"), "{}", fixture.log());
    fixture.assert_clean(&ran);
}

#[test]
fn notarize_offline_or_still_in_progress_exits_3() {
    let fixture = Fixture::new("apple-notarize-offline");
    fixture.signed_app();
    let ran = fixture.notarize(&KEY, &[("FAKE_OFFLINE", "1")]);
    assert_eq!(ran.status.code(), Some(3), "{}", ran.combined());
    assert!(
        ran.stderr.ends_with(
            "dx-apple-notarize: the Apple notary service is not reachable; retry when online\n"
        ),
        "{}",
        ran.combined()
    );
    assert!(
        !fixture.log().contains("notarytool log"),
        "{}",
        fixture.log()
    );
    fixture.assert_clean(&ran);

    let pending = Fixture::new("apple-notarize-pending");
    pending.signed_app();
    let ran = pending.notarize(&KEY, &[("FAKE_STATUS", "In Progress")]);
    fails(
        &ran,
        "dx-apple-notarize",
        3,
        &format!("submission {SUBMISSION} is still In Progress after --timeout 2h; check it with xcrun notarytool info {SUBMISSION}"),
    );
    assert!(!pending.log().contains("stapler"), "{}", pending.log());
}

#[test]
fn notarize_failed_staple_or_assessment_exits_1() {
    for (env, want) in [
        (
            ("FAKE_STAPLE_EXIT", "65"),
            "cannot staple the notarization ticket to Ledger.app",
        ),
        (
            ("FAKE_VALIDATE_EXIT", "65"),
            "stapled Ledger.app fails stapler validate",
        ),
    ] {
        let fixture = Fixture::new("apple-notarize-staple");
        fixture.signed_app();
        let ran = fixture.notarize(&KEY, &[env]);
        fails(&ran, "dx-apple-notarize", 1, want);
    }
    let fixture = Fixture::new("apple-notarize-spctl");
    fixture.signed_app();
    let ran = fixture.notarize(&KEY, &[("FAKE_SPCTL_EXIT", "3")]);
    assert_eq!(ran.status.code(), Some(1), "{}", ran.combined());
    assert!(
        ran.stderr.ends_with(
            "dx-apple-notarize: stapled Ledger.app fails spctl --assess --type execute\n"
        ),
        "{}",
        ran.combined()
    );
}

#[test]
fn notarize_rejects_bad_input_before_submitting() {
    let fixture = Fixture::new("apple-notarize-args");
    fixture.signed_app();
    let usage = "usage: bazel run <notarize> -- --app <App.app> (--keychain-profile <name> | --key <AuthKey.p8> --key-id <id> --issuer <uuid>) [--timeout <duration>]";
    let ws = fixture.path("ws");
    let cases: Vec<Case> = vec![
        (
            KEY[2..].to_vec(),
            vec![],
            format!("--app names the signed .app to notarize; {usage}"),
        ),
        (
            KEY[..2].to_vec(),
            vec![],
            format!("pass --keychain-profile or --key with --key-id and --issuer; {usage}"),
        ),
        (
            [&KEY[..], &["--keychain-profile", "ledger-notary"][..]].concat(),
            vec![],
            format!(
                "pass --keychain-profile or --key with --key-id and --issuer, not both; {usage}"
            ),
        ),
        (
            KEY[..6].to_vec(),
            vec![],
            format!("--key, --key-id and --issuer go together; {usage}"),
        ),
        (
            [&KEY[..2], &["--key", "Missing.p8"][..], &KEY[4..]].concat(),
            vec![],
            format!("--key {ws}/Missing.p8 is not a file"),
        ),
        (
            [&["--app", "dist/Other.app"][..], &KEY[2..]].concat(),
            vec![],
            format!("--app {ws}/dist/Other.app is not a bundle directory"),
        ),
        (
            KEY.to_vec(),
            vec![("FAKE_ADHOC", "1")],
            "Ledger.app carries an ad hoc signature; run apple_macos_sign first".to_owned(),
        ),
        (
            KEY.to_vec(),
            vec![("FAKE_FLAGS", "0x0(none)")],
            "Ledger.app is not signed with the hardened runtime; run apple_macos_sign first"
                .to_owned(),
        ),
    ];
    for (args, envs, want) in cases {
        let ran = fixture.notarize(&args, &envs);
        fails(&ran, "dx-apple-notarize", 2, &want);
        fixture.assert_clean(&ran);
    }
    write(
        &fixture.root.join("ws/dist/Ledger.app/Contents/Info.plist"),
        "CFBundleExecutable=Ledger\nCFBundleIdentifier=com.example.other\n",
    );
    let ran = fixture.notarize(&KEY, &[]);
    fails(
        &ran,
        "dx-apple-notarize",
        2,
        "Ledger.app is com.example.other, not the declared com.example.ledger",
    );
    assert!(!fixture.log().contains("notarytool"), "{}", fixture.log());
    assert!(!fixture.log().contains("ditto"), "{}", fixture.log());
}
