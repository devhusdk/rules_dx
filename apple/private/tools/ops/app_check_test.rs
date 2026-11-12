//! Tests the macOS app bundle check.

mod fake;

use std::io::Write;
use std::path::{Path, PathBuf};

use dx_apple_ops::app_check::{parse_loads, run, sha256};
use dx_apple_ops::util::Output;
use dx_apple_ops::xcode::{CODESIGN, PLUTIL, Tools, XCRUN};
use dx_test_scratch::scratch;
use fake::{Fake, args, fail, ok};

fn loads(extra: &str) -> String {
    format!(
        "Load command 9\n      cmd LC_BUILD_VERSION\n  cmdsize 32\n platform 1\n    minos 14.0\n      sdk 15.5\n{extra}"
    )
}

fn load(name: &str) -> String {
    format!(
        "Load command 11\n          cmd LC_LOAD_DYLIB\n      cmdsize 56\n         name {name} (offset 24)\n"
    )
}

fn rpath(path: &str) -> String {
    format!(
        "Load command 12\n          cmd LC_RPATH\n      cmdsize 40\n         path {path} (offset 12)\n"
    )
}

fn mach_o(archs: &str, loads: &str) -> Vec<u8> {
    format!("MACHO {archs}\n{loads}").into_bytes()
}

fn plist(bundle_id: &str, version: &str) -> Vec<u8> {
    format!(
        "CFBundleIdentifier={bundle_id}\nCFBundleExecutable=Ledger\nCFBundleShortVersionString={version}\nCFBundleVersion={version}\nLSMinimumSystemVersion=14.0\nCFBundlePackageType=APPL\nCFBundleSupportedPlatforms.0=MacOSX\n"
    )
    .into_bytes()
}

enum Entry {
    File(Vec<u8>),
    Link(String),
}

struct Bundle(Vec<(String, Entry)>);

impl Bundle {
    fn new() -> Bundle {
        let mut bundle = Bundle(Vec::new());
        bundle.file("Info.plist", plist("com.example.ledger", "1.0"));
        bundle.file(
            "MacOS/Ledger",
            mach_o("arm64", &loads(&load("/usr/lib/libSystem.B.dylib"))),
        );
        bundle.file("Resources/rates.txt", b"tea 450\n".to_vec());
        bundle
    }

    fn file(&mut self, path: &str, bytes: Vec<u8>) {
        self.0.retain(|(have, _)| have != path);
        self.0.push((path.to_owned(), Entry::File(bytes)));
    }

    fn link(&mut self, path: &str, target: &str) {
        self.0
            .push((path.to_owned(), Entry::Link(target.to_owned())));
    }

    fn framework(&mut self, name: &str, versioned: bool, exe_loads: &str) {
        let base = format!("Frameworks/{name}.framework");
        if versioned {
            self.file(
                &format!("{base}/Versions/A/{name}"),
                mach_o("arm64", &loads(exe_loads)),
            );
            self.file(
                &format!("{base}/Versions/A/Resources/Info.plist"),
                format!("CFBundleExecutable={name}\n").into_bytes(),
            );
            self.link(&format!("{base}/Versions/Current"), "A");
        } else {
            self.file(
                &format!("{base}/{name}"),
                mach_o("arm64", &loads(exe_loads)),
            );
            self.file(
                &format!("{base}/Info.plist"),
                format!("CFBundleExecutable={name}\n").into_bytes(),
            );
        }
    }

    fn write(&self, path: &Path) {
        let mut writer = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        for (name, entry) in &self.0 {
            let full = format!("Ledger.app/Contents/{name}");
            match entry {
                Entry::File(bytes) => {
                    writer.start_file(full, options).unwrap();
                    writer.write_all(bytes).unwrap();
                }
                Entry::Link(target) => writer.add_symlink(full, target, options).unwrap(),
            }
        }
        writer.finish().unwrap();
    }
}

struct Checked {
    outcome: Result<u8, String>,
    identity: PathBuf,
    out: PathBuf,
    root: dx_test_scratch::TempDir,
}

struct Want<'a> {
    bundle_id: &'a str,
    version: &'a str,
    frameworks: &'a str,
    plugins: &'a str,
    resources: &'a str,
    codesign: &'a str,
}

const WANT: Want = Want {
    bundle_id: "com.example.ledger",
    version: "1.0",
    frameworks: "",
    plugins: "",
    resources: "rates.txt",
    codesign: "Executable=/x/Ledger\nSignature=adhoc\n",
};

fn read(path: &str) -> Option<Vec<u8>> {
    std::fs::read(path).ok()
}

fn tool(cmd: &dx_apple_ops::util::Cmd, codesign: &str) -> Output {
    if cmd.program == CODESIGN {
        return Output {
            code: Some(0),
            stdout: String::new(),
            stderr: codesign.to_owned(),
        };
    }
    if cmd.program == PLUTIL {
        let key = &cmd.args[1];
        let text = String::from_utf8(read(&cmd.args[5]).unwrap_or_default()).unwrap_or_default();
        return match text
            .lines()
            .find_map(|line| line.strip_prefix(&format!("{key}=")))
        {
            Some(value) => ok(&format!("{value}\n")),
            None => fail(1, "No value at that key path"),
        };
    }
    assert_eq!(cmd.program, XCRUN);
    let file = cmd.args.last().unwrap();
    let text = String::from_utf8(read(file).unwrap_or_default()).unwrap_or_default();
    let Some(rest) = text.strip_prefix("MACHO ") else {
        return fail(1, "not a Mach-O");
    };
    let (archs, body) = rest.split_once('\n').unwrap_or((rest, ""));
    match cmd.args.first().map(String::as_str) {
        Some("lipo") => ok(&format!("{archs}\n")),
        Some("otool") => ok(body),
        other => panic!("unexpected {other:?}"),
    }
}

fn check_with(bundle: &Bundle, want: &Want, arch: &str) -> Checked {
    let root = scratch("macos-app-check");
    let out_dir = root.path().join("out");
    std::fs::create_dir_all(&out_dir).unwrap();
    let archive = root.path().join("Ledger_bundle.zip");
    bundle.write(&archive);
    let identity = out_dir.join("Ledger.app.identity.json");
    let out = out_dir.join("Ledger.zip");
    let codesign = want.codesign.to_owned();
    let fake = Fake::new(move |cmd| tool(cmd, &codesign));
    let tools = Tools {
        developer_dir: None,
        runner: &fake,
    };
    let identity_text = identity.to_string_lossy().into_owned();
    let out_text = out.to_string_lossy().into_owned();
    let archive_text = archive.to_string_lossy().into_owned();
    let argv = args(&[
        &identity_text,
        &out_text,
        &archive_text,
        "Ledger",
        ".app",
        "Ledger",
        want.bundle_id,
        want.version,
        "14.0",
        arch,
        "16.4.0.16F6",
        "15.5",
        want.frameworks,
        want.plugins,
        want.resources,
    ]);
    let outcome = run(&tools, &argv).map_err(|failure| failure.message);
    Checked {
        outcome,
        identity,
        out,
        root,
    }
}

fn check(bundle: &Bundle) -> Checked {
    check_with(bundle, &WANT, "arm64")
}

fn rejects(checked: &Checked, want: &str) {
    assert_eq!(
        checked.outcome.as_ref().err().map(String::as_str),
        Some(want)
    );
    assert!(!checked.identity.exists(), "{want}: identity was written");
    assert!(!checked.out.exists(), "{want}: a rejected app is published");
    let left: Vec<_> = std::fs::read_dir(checked.root.path().join("out"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    assert!(left.is_empty(), "{want}: left {left:?}");
}

fn json(checked: &Checked) -> String {
    assert_eq!(checked.outcome, Ok(0));
    std::fs::read_to_string(&checked.identity).unwrap()
}

#[test]
fn sha256_matches_the_known_digests() {
    assert_eq!(
        sha256(b""),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    assert_eq!(
        sha256(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    assert_eq!(
        sha256(&[b'a'; 1000]),
        "41edece42d63e8d9bf515a9ba6932e1c20cbc9f5a5d134645adb5db1b9737ea3"
    );
}

#[test]
fn load_commands_are_read_in_order() {
    let found = parse_loads(&loads(&format!(
        "{}{}",
        load("@rpath/Core.framework/Core"),
        rpath("@executable_path/../Frameworks")
    )));
    assert_eq!(found.platforms.len(), 1);
    assert_eq!(found.minos, ["14.0"]);
    assert_eq!(found.loads, ["@rpath/Core.framework/Core"]);
    assert_eq!(found.rpaths, ["@executable_path/../Frameworks"]);
}

#[test]
fn a_truthful_app_writes_its_identity_and_publishes_the_zip() {
    let mut bundle = Bundle::new();
    bundle.framework("Core", false, &load("/usr/lib/libc++.1.dylib"));
    bundle.file(
        "MacOS/Ledger",
        mach_o(
            "arm64",
            &loads(&format!(
                "{}{}{}",
                load("/usr/lib/libSystem.B.dylib"),
                load("@rpath/Core.framework/Core"),
                rpath("@executable_path/../Frameworks")
            )),
        ),
    );
    bundle.file(
        "PlugIns/Share.appex/Contents/MacOS/Share",
        mach_o(
            "arm64",
            &loads(&load(
                "/System/Library/Frameworks/Foundation.framework/Foundation",
            )),
        ),
    );
    let want = Want {
        frameworks: "Core\n",
        plugins: "Share.appex\n",
        ..WANT
    };
    let checked = check_with(&bundle, &want, "arm64");
    let executable = mach_o(
        "arm64",
        &loads(&format!(
            "{}{}{}",
            load("/usr/lib/libSystem.B.dylib"),
            load("@rpath/Core.framework/Core"),
            rpath("@executable_path/../Frameworks")
        )),
    );
    let expected = format!(
        "{{\n  \"identity\": {{\n    \"bundle_id\": \"com.example.ledger\",\n    \"executable\": \"Ledger\",\n    \"version\": \"1.0\",\n    \"deployment\": \"14.0\"\n  }},\n  \"target\": {{\n    \"arch\": \"arm64\",\n    \"platform\": \"macosx\",\n    \"sdk_version\": \"15.5\",\n    \"xcode\": \"16.4.0.16F6\",\n    \"minos\": \"14.0\"\n  }},\n  \"linkage\": {{\n    \"rpaths\": [\"@executable_path/../Frameworks\"],\n    \"loads\": [\"/System/Library/Frameworks/Foundation.framework/Foundation\", \"/usr/lib/libSystem.B.dylib\", \"/usr/lib/libc++.1.dylib\", \"@rpath/Core.framework/Core\"]\n  }},\n  \"contents\": {{\n    \"frameworks\": [\"Core\"],\n    \"plugins\": [\"Share.appex\"],\n    \"resources\": [\"rates.txt\"]\n  }},\n  \"signing\": {{\n    \"signature\": \"adhoc\",\n    \"executable_sha256\": \"{}\"\n  }}\n}}\n",
        sha256(&executable)
    );
    assert_eq!(json(&checked), expected);
    let published = std::fs::read(&checked.out).unwrap();
    let archive = std::fs::read(checked.root.path().join("Ledger_bundle.zip")).unwrap();
    assert_eq!(published, archive);
    let left: Vec<_> = std::fs::read_dir(checked.root.path().join("out"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    assert_eq!(left.len(), 2, "{left:?}");
}

#[test]
fn a_versioned_framework_layout_passes() {
    let mut bundle = Bundle::new();
    bundle.framework("Core", true, &load("/usr/lib/libc++.1.dylib"));
    bundle.file(
        "MacOS/Ledger",
        mach_o(
            "arm64",
            &loads(&load("@rpath/Core.framework/Versions/A/Core")),
        ),
    );
    let want = Want {
        frameworks: "Core\n",
        resources: "",
        ..WANT
    };
    let checked = check_with(&bundle, &want, "arm64");
    assert_eq!(checked.outcome, Ok(0));
    assert!(checked.identity.exists());
}

#[test]
fn a_missing_framework_plugin_or_resource_fails() {
    let bundle = Bundle::new();
    for (want, message) in [
        (
            Want {
                frameworks: "Core\n",
                ..WANT
            },
            "declared framework Core.framework is missing from the bundle",
        ),
        (
            Want {
                plugins: "Share.appex\n",
                ..WANT
            },
            "declared plugin Share.appex is missing from the bundle",
        ),
        (
            Want {
                resources: "missing.txt\n",
                ..WANT
            },
            "declared resource missing.txt is missing from the bundle",
        ),
    ] {
        rejects(&check_with(&bundle, &want, "arm64"), message);
    }
}

#[test]
fn a_wrong_arch_on_the_executable_or_a_plugin_fails() {
    let mut bundle = Bundle::new();
    bundle.file("MacOS/Ledger", mach_o("x86_64", &loads("")));
    rejects(
        &check(&bundle),
        "Ledger.app/Contents/MacOS/Ledger carries 'x86_64', want arm64",
    );
    let mut bundle = Bundle::new();
    bundle.file(
        "PlugIns/Share.appex/Contents/MacOS/Share",
        mach_o("arm64 x86_64", &loads("")),
    );
    rejects(
        &check(&bundle),
        "Ledger.app/Contents/PlugIns/Share.appex/Contents/MacOS/Share carries 'arm64 x86_64', want arm64",
    );
}

#[test]
fn host_loads_absolute_rpaths_and_unresolved_loads_fail() {
    let cases = [
        (
            load("/opt/homebrew/lib/libssl.3.dylib"),
            "Ledger.app/Contents/MacOS/Ledger loads host runtime /opt/homebrew/lib/libssl.3.dylib; embed it under Frameworks",
        ),
        (
            rpath("/opt/homebrew/lib"),
            "Ledger.app/Contents/MacOS/Ledger has absolute rpath /opt/homebrew/lib; use @executable_path or @loader_path",
        ),
        (
            load("@rpath/Core.framework/Core"),
            "Ledger.app/Contents/MacOS/Ledger loads @rpath/Core.framework/Core, which is missing from the bundle",
        ),
    ];
    for (extra, message) in cases {
        let mut bundle = Bundle::new();
        bundle.file("MacOS/Ledger", mach_o("arm64", &loads(&extra)));
        rejects(&check(&bundle), message);
    }
}

#[test]
fn a_plist_mismatch_or_a_bad_platform_or_minos_fails() {
    let want = Want {
        version: "2.0",
        ..WANT
    };
    rejects(
        &check_with(&Bundle::new(), &want, "arm64"),
        "Ledger.app has CFBundleShortVersionString '1.0', want '2.0'",
    );
    let mut bundle = Bundle::new();
    bundle.file(
        "MacOS/Ledger",
        mach_o("arm64", &loads("").replace("platform 1", "platform 2")),
    );
    rejects(
        &check(&bundle),
        "Ledger.app/Contents/MacOS/Ledger declares build platform '2', want 1 for macosx",
    );
    let mut bundle = Bundle::new();
    bundle.file(
        "MacOS/Ledger",
        mach_o("arm64", &loads("").replace("minos 14.0", "minos 15.0")),
    );
    rejects(
        &check(&bundle),
        "Ledger.app/Contents/MacOS/Ledger needs macOS 15.0, above the app deployment 14.0",
    );
}

#[test]
fn a_non_adhoc_signature_fails() {
    let want = Want {
        codesign: "Authority=Developer ID Application: Example\nSignature size=9000\n",
        ..WANT
    };
    rejects(
        &check_with(&Bundle::new(), &want, "arm64"),
        "Ledger.app has signature 'none', want adhoc",
    );
}

#[test]
fn a_bad_archive_fails_without_publishing() {
    let root = scratch("macos-app-zip");
    std::fs::create_dir_all(root.path().join("out")).unwrap();
    let archive = root.path().join("Ledger_bundle.zip");
    std::fs::write(&archive, "not a zip").unwrap();
    let fake = Fake::new(|_| fail(1, ""));
    let tools = Tools {
        developer_dir: None,
        runner: &fake,
    };
    let identity = root.path().join("out/Ledger.app.identity.json");
    let out = root.path().join("out/Ledger.zip");
    let argv = args(&[
        &identity.to_string_lossy(),
        &out.to_string_lossy(),
        &archive.to_string_lossy(),
        "Ledger",
        ".app",
        "Ledger",
        "com.example.ledger",
        "1.0",
        "14.0",
        "arm64",
        "16.4.0.16F6",
        "15.5",
        "",
        "",
        "",
    ]);
    let message = run(&tools, &argv).unwrap_err().message;
    assert!(
        message.starts_with("Ledger_bundle.zip does not unpack: "),
        "{message}"
    );
    assert!(!identity.exists() && !out.exists());
}

#[test]
fn identity_strings_are_escaped() {
    let mut bundle = Bundle::new();
    bundle.file("Info.plist", plist("com.example.\"led\\ger", "1.0"));
    let want = Want {
        bundle_id: "com.example.\"led\\ger",
        ..WANT
    };
    let checked = check_with(&bundle, &want, "arm64");
    assert!(json(&checked).contains("\"bundle_id\": \"com.example.\\\"led\\\\ger\""));
}

#[test]
fn too_few_arguments_is_a_usage_error() {
    let fake = Fake::new(|_| fail(1, ""));
    let tools = Tools {
        developer_dir: None,
        runner: &fake,
    };
    let failure = run(&tools, &args(&["x"])).unwrap_err();
    assert_eq!(failure.code, 2);
}
