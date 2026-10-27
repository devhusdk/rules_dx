use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use dx_testing::{mkscratch, read_json, resolve_runfiles, run, Run};

const FAKE_UNZIP: &str = r#"#!/bin/sh
if [ -n "${FAKE_BAD_ZIP:-}" ]; then
    echo "unzip: cannot find zipfile directory" >&2
    exit 9
fi
cp -R "$FAKE_TREE/." "$4"
"#;

const FAKE_PLUTIL: &str = r#"#!/bin/sh
awk -v key="$2" 'index($0, key "=") == 1 { print substr($0, length(key) + 2); found = 1 } END { if (!found) print "No value at that key path or invalid key path: " key; exit !found }' "$6"
"#;

const FAKE_XCRUN: &str = r#"#!/bin/sh
case "$1" in
    lipo)
        cat "$3.archs" 2>/dev/null
        ;;
    otool)
        cat "$5.$3.loads"
        ;;
    *)
        exit 64
        ;;
esac
"#;

const FAKE_CODESIGN: &str = r#"#!/bin/sh
if [ -n "${FAKE_CODESIGN_OUT:-}" ]; then
    printf '%s\n' "$FAKE_CODESIGN_OUT" >&2
else
    echo "Executable=/x/Ledger" >&2
    echo "Signature=adhoc" >&2
fi
"#;

const FAKE_SHASUM: &str = r#"#!/bin/sh
echo "c0ffee  $3"
"#;

const FAKE_DITTO: &str = r#"#!/bin/sh
cp -R "$FAKE_TREE/." "$4"
"#;

const NAME: &str = "Ledger";
const ZIP_BYTES: &str = "PK\u{3}\u{4}ledger";

fn list(items: &[&str]) -> String {
    let quoted: Vec<String> = items.iter().map(|item| format!("\"{item}\"")).collect();
    format!("[{}]", quoted.join(","))
}

fn write(path: &Path, text: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap_or_else(|error| panic!("mkdir: {error}"));
    }
    std::fs::write(path, text).unwrap_or_else(|error| panic!("write {path:?}: {error}"));
}

fn executable(path: &Path, text: &str) {
    write(path, text);
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
        .unwrap_or_else(|error| panic!("chmod: {error}"));
}

fn loads(extra: &str) -> String {
    format!(
        "Load command 9\n      cmd LC_BUILD_VERSION\n  cmdsize 32\n platform 1\n    minos 14.0\n      sdk 15.5\n{extra}"
    )
}

fn load(name: &str) -> String {
    format!("Load command 11\n          cmd LC_LOAD_DYLIB\n      cmdsize 56\n         name {name} (offset 24)\n")
}

fn rpath(path: &str) -> String {
    format!("Load command 12\n          cmd LC_RPATH\n      cmdsize 40\n         path {path} (offset 12)\n")
}

fn mach_o(path: &Path, archs: &str, loads: &str) {
    write(path, "mach-o");
    let name = path.file_name().unwrap().to_string_lossy().into_owned();
    write(&path.with_file_name(format!("{name}.archs")), archs);
    for arch in archs.split(' ') {
        write(&path.with_file_name(format!("{name}.{arch}.loads")), loads);
    }
}

fn plist(bundle_id: &str, version: &str) -> String {
    format!("CFBundleIdentifier={bundle_id}\nCFBundleExecutable=Ledger\nCFBundleShortVersionString={version}\nCFBundleVersion={version}\nLSMinimumSystemVersion=14.0\nCFBundlePackageType=APPL\nCFBundleSupportedPlatforms.0=MacOSX\n")
}

struct Case {
    root: PathBuf,
    contents: PathBuf,
}

impl Case {
    fn new(prefix: &str) -> Case {
        let root = mkscratch(prefix).unwrap_or_else(|error| panic!("scratch: {error}"));
        let bin = root.join("bin");
        executable(&bin.join("unzip"), FAKE_UNZIP);
        executable(&bin.join("plutil"), FAKE_PLUTIL);
        executable(&bin.join("xcrun"), FAKE_XCRUN);
        executable(&bin.join("codesign"), FAKE_CODESIGN);
        executable(&bin.join("shasum"), FAKE_SHASUM);
        let contents = root.join("tree/Ledger.app/Contents");
        write(
            &contents.join("Info.plist"),
            &plist("com.example.ledger", "1.0"),
        );
        mach_o(
            &contents.join("MacOS/Ledger"),
            "arm64",
            &loads(&load("/usr/lib/libSystem.B.dylib")),
        );
        write(&contents.join("Resources/rates.txt"), "tea 450\n");
        write(&root.join("Ledger_bundle.zip"), ZIP_BYTES);
        Case { root, contents }
    }

    fn identity(&self) -> PathBuf {
        self.root.join("out/Ledger.app.identity.json")
    }

    fn out(&self) -> PathBuf {
        self.root.join("out/Ledger.zip")
    }

    fn framework(&self, name: &str, versioned: bool, exe_loads: &str) {
        let bundle = self.contents.join(format!("Frameworks/{name}.framework"));
        if versioned {
            let version = bundle.join("Versions/A");
            mach_o(&version.join(name), "arm64", &loads(exe_loads));
            write(
                &version.join("Resources/Info.plist"),
                &format!("CFBundleExecutable={name}\n"),
            );
            std::os::unix::fs::symlink("A", bundle.join("Versions/Current"))
                .unwrap_or_else(|error| panic!("symlink: {error}"));
            std::os::unix::fs::symlink("Versions/Current/Resources", bundle.join("Resources"))
                .unwrap_or_else(|error| panic!("symlink: {error}"));
            std::os::unix::fs::symlink(format!("Versions/Current/{name}"), bundle.join(name))
                .unwrap_or_else(|error| panic!("symlink: {error}"));
        } else {
            mach_o(&bundle.join(name), "arm64", &loads(exe_loads));
            write(
                &bundle.join("Info.plist"),
                &format!("CFBundleExecutable={name}\n"),
            );
        }
    }

    fn check(&self, envs: &[(&str, &str)]) -> Run {
        self.check_with(envs, "com.example.ledger", "1.0", "", "", "rates.txt")
    }

    fn check_with(
        &self,
        envs: &[(&str, &str)],
        bundle_id: &str,
        version: &str,
        frameworks: &str,
        plugins: &str,
        resources: &str,
    ) -> Run {
        std::fs::create_dir_all(self.root.join("out"))
            .unwrap_or_else(|error| panic!("mkdir: {error}"));
        let path = format!(
            "{}:{}",
            self.root.join("bin").display(),
            std::env::var("PATH").unwrap_or_default()
        );
        let identity = self.identity().to_string_lossy().into_owned();
        let out = self.out().to_string_lossy().into_owned();
        let archive = self
            .root
            .join("Ledger_bundle.zip")
            .to_string_lossy()
            .into_owned();
        let tree = self.root.join("tree").to_string_lossy().into_owned();
        let mut all = vec![("PATH", path.as_str()), ("FAKE_TREE", tree.as_str())];
        all.extend_from_slice(envs);
        let script = resolve_runfiles(
            &std::env::var("DX_MACOS_APP_CHECK")
                .unwrap_or_else(|_| panic!("DX_MACOS_APP_CHECK must name a script")),
        );
        run(
            &script,
            &[
                identity.as_str(),
                out.as_str(),
                archive.as_str(),
                NAME,
                ".app",
                "Ledger",
                bundle_id,
                version,
                "14.0",
                "arm64",
                "16.4.0.16F6",
                "15.5",
                frameworks,
                plugins,
                resources,
            ],
            &all,
        )
        .unwrap_or_else(|error| panic!("run: {error}"))
    }

    fn rejects(&self, ran: Run, want: &str) {
        assert_eq!(ran.status.code(), Some(1), "{}", ran.combined());
        assert_eq!(ran.stderr, format!("apple_macos_app: {want}\n"));
        assert!(!self.identity().exists(), "{want}: identity was written");
        assert!(!self.out().exists(), "{want}: a rejected app is published");
        assert!(
            !self
                .root
                .join("out/Ledger.app.identity.json.scratch")
                .exists(),
            "{want}: the unpacked copy outlived the check"
        );
    }
}

#[test]
fn a_truthful_app_writes_its_identity_and_publishes_the_zip() {
    let case = Case::new("macos-app-ok");
    case.framework("Core", false, &load("/usr/lib/libc++.1.dylib"));
    mach_o(
        &case.contents.join("MacOS/Ledger"),
        "arm64",
        &loads(&format!(
            "{}{}{}",
            load("/usr/lib/libSystem.B.dylib"),
            load("@rpath/Core.framework/Core"),
            rpath("@executable_path/../Frameworks")
        )),
    );
    mach_o(
        &case
            .contents
            .join("PlugIns/Share.appex/Contents/MacOS/Share"),
        "arm64",
        &loads(&load(
            "/System/Library/Frameworks/Foundation.framework/Foundation",
        )),
    );
    let ran = case.check_with(
        &[],
        "com.example.ledger",
        "1.0",
        "Core",
        "Share.appex",
        "rates.txt",
    );
    assert_eq!(ran.status.code(), Some(0), "{}", ran.combined());
    assert_eq!(
        std::fs::read_to_string(case.out()).unwrap_or_default(),
        ZIP_BYTES
    );
    let json = read_json(&case.identity()).unwrap_or_else(|error| panic!("json: {error}"));
    assert_eq!(json["identity"]["bundle_id"], "com.example.ledger");
    assert_eq!(json["identity"]["executable"], "Ledger");
    assert_eq!(json["identity"]["deployment"], "14.0");
    assert_eq!(json["target"]["arch"], "arm64");
    assert_eq!(json["target"]["platform"], "macosx");
    assert_eq!(json["target"]["sdk_version"], "15.5");
    assert_eq!(json["target"]["xcode"], "16.4.0.16F6");
    assert_eq!(json["target"]["minos"], "14.0");
    assert_eq!(
        json["linkage"]["rpaths"].to_string(),
        list(&["@executable_path/../Frameworks"])
    );
    assert_eq!(
        json["linkage"]["loads"].to_string(),
        list(&[
            "/System/Library/Frameworks/Foundation.framework/Foundation",
            "/usr/lib/libSystem.B.dylib",
            "/usr/lib/libc++.1.dylib",
            "@rpath/Core.framework/Core"
        ])
    );
    assert_eq!(json["contents"]["frameworks"].to_string(), list(&["Core"]));
    assert_eq!(
        json["contents"]["plugins"].to_string(),
        list(&["Share.appex"])
    );
    assert_eq!(
        json["contents"]["resources"].to_string(),
        list(&["rates.txt"])
    );
    assert_eq!(json["signing"]["signature"], "adhoc");
    assert_eq!(json["signing"]["executable_sha256"], "c0ffee");
}

#[test]
fn a_versioned_framework_layout_passes() {
    let case = Case::new("macos-app-versioned");
    case.framework("Core", true, &load("/usr/lib/libc++.1.dylib"));
    mach_o(
        &case.contents.join("MacOS/Ledger"),
        "arm64",
        &loads(&load("@rpath/Core.framework/Versions/A/Core")),
    );
    let ran = case.check_with(&[], "com.example.ledger", "1.0", "Core", "", "");
    assert_eq!(ran.status.code(), Some(0), "{}", ran.combined());
    assert!(case.identity().exists());
}

#[test]
fn a_missing_framework_plugin_or_resource_fails() {
    let case = Case::new("macos-app-missing");
    let ran = case.check_with(&[], "com.example.ledger", "1.0", "Core", "", "");
    case.rejects(
        ran,
        "declared framework Core.framework is missing from the bundle",
    );
    let ran = case.check_with(&[], "com.example.ledger", "1.0", "", "Share.appex", "");
    case.rejects(
        ran,
        "declared plugin Share.appex is missing from the bundle",
    );
    let ran = case.check_with(&[], "com.example.ledger", "1.0", "", "", "missing.txt");
    case.rejects(
        ran,
        "declared resource missing.txt is missing from the bundle",
    );
}

#[test]
fn a_wrong_arch_on_the_executable_or_a_plugin_fails() {
    let case = Case::new("macos-app-arch");
    mach_o(&case.contents.join("MacOS/Ledger"), "arm64", &loads(""));
    mach_o(&case.contents.join("MacOS/Ledger"), "x86_64", &loads(""));
    case.rejects(
        case.check(&[]),
        "Ledger.app/Contents/MacOS/Ledger carries 'x86_64', want arm64",
    );
    let case = Case::new("macos-app-plugin-arch");
    mach_o(
        &case
            .contents
            .join("PlugIns/Share.appex/Contents/MacOS/Share"),
        "arm64 x86_64",
        &loads(""),
    );
    case.rejects(
        case.check(&[]),
        "Ledger.app/Contents/PlugIns/Share.appex/Contents/MacOS/Share carries 'arm64 x86_64', want arm64",
    );
}

#[test]
fn host_loads_absolute_rpaths_and_unresolved_loads_fail() {
    let case = Case::new("macos-app-host");
    mach_o(
        &case.contents.join("MacOS/Ledger"),
        "arm64",
        &loads(&load("/opt/homebrew/lib/libssl.3.dylib")),
    );
    case.rejects(
        case.check(&[]),
        "Ledger.app/Contents/MacOS/Ledger loads host runtime /opt/homebrew/lib/libssl.3.dylib; embed it under Frameworks",
    );
    let case = Case::new("macos-app-rpath");
    mach_o(
        &case.contents.join("MacOS/Ledger"),
        "arm64",
        &loads(&rpath("/opt/homebrew/lib")),
    );
    case.rejects(
        case.check(&[]),
        "Ledger.app/Contents/MacOS/Ledger has absolute rpath /opt/homebrew/lib; use @executable_path or @loader_path",
    );
    let case = Case::new("macos-app-unresolved");
    mach_o(
        &case.contents.join("MacOS/Ledger"),
        "arm64",
        &loads(&load("@rpath/Core.framework/Core")),
    );
    case.rejects(
        case.check(&[]),
        "Ledger.app/Contents/MacOS/Ledger loads @rpath/Core.framework/Core, which is missing from the bundle",
    );
}

#[test]
fn a_plist_mismatch_or_a_bad_platform_or_minos_fails() {
    let case = Case::new("macos-app-plist");
    let ran = case.check_with(&[], "com.example.ledger", "2.0", "", "", "rates.txt");
    case.rejects(
        ran,
        "Ledger.app has CFBundleShortVersionString '1.0', want '2.0'",
    );
    let case = Case::new("macos-app-platform");
    mach_o(
        &case.contents.join("MacOS/Ledger"),
        "arm64",
        &loads("").replace("platform 1", "platform 2"),
    );
    case.rejects(
        case.check(&[]),
        "Ledger.app/Contents/MacOS/Ledger declares build platform '2', want 1 for macosx",
    );
    let case = Case::new("macos-app-minos");
    mach_o(
        &case.contents.join("MacOS/Ledger"),
        "arm64",
        &loads("").replace("minos 14.0", "minos 15.0"),
    );
    case.rejects(
        case.check(&[]),
        "Ledger.app/Contents/MacOS/Ledger needs macOS 15.0, above the app deployment 14.0",
    );
}

#[test]
fn a_non_adhoc_signature_fails() {
    let case = Case::new("macos-app-signature");
    let ran = case.check(&[(
        "FAKE_CODESIGN_OUT",
        "Authority=Developer ID Application: Example\nSignature size=9000",
    )]);
    case.rejects(ran, "Ledger.app has signature 'none', want adhoc");
}

#[test]
fn a_bad_archive_fails_without_publishing() {
    let case = Case::new("macos-app-zip");
    let ran = case.check(&[("FAKE_BAD_ZIP", "1")]);
    case.rejects(
        ran,
        "Ledger_bundle.zip does not unpack: unzip: cannot find zipfile directory",
    );
}

#[test]
fn identity_strings_are_escaped() {
    let case = Case::new("macos-app-escape");
    write(
        &case.contents.join("Info.plist"),
        &plist("com.example.\"led\\ger", "1.0"),
    );
    let ran = case.check_with(&[], "com.example.\"led\\ger", "1.0", "", "", "rates.txt");
    assert_eq!(ran.status.code(), Some(0), "{}", ran.combined());
    let json = read_json(&case.identity()).unwrap_or_else(|error| panic!("json: {error}"));
    assert_eq!(json["identity"]["bundle_id"], "com.example.\"led\\ger");
}

#[test]
fn the_launcher_exits_127_without_ditto_and_runs_the_app_with_it() {
    let case = Case::new("macos-app-run");
    let launcher = resolve_runfiles(
        &std::env::var("DX_MACOS_RUN_APP")
            .unwrap_or_else(|_| panic!("DX_MACOS_RUN_APP must name a script")),
    );
    let empty = case.root.join("empty");
    std::fs::create_dir_all(&empty).unwrap_or_else(|error| panic!("mkdir: {error}"));
    let empty_path = empty.to_string_lossy().into_owned();
    let ran = run(&launcher, &[], &[("PATH", empty_path.as_str())])
        .unwrap_or_else(|error| panic!("run: {error}"));
    assert_eq!(ran.status.code(), Some(127), "{}", ran.combined());
    assert_eq!(
        ran.stderr,
        "apple_macos_app: no ditto on PATH; running the app needs macOS\n"
    );

    executable(
        &case.contents.join("MacOS/Ledger"),
        "#!/bin/sh\necho \"ledger $*\"\n",
    );
    executable(&case.root.join("bin/ditto"), FAKE_DITTO);
    let runfiles = case.root.join("runfiles");
    write(&runfiles.join("_main/pkg/Ledger.zip"), ZIP_BYTES);
    let path = format!("{}:/usr/bin:/bin", case.root.join("bin").display());
    let tree = case.root.join("tree").to_string_lossy().into_owned();
    let runfiles = runfiles.to_string_lossy().into_owned();
    let tmp = case.root.to_string_lossy().into_owned();
    let ran = run(
        &launcher,
        &["--flag"],
        &[
            ("PATH", path.as_str()),
            ("FAKE_TREE", tree.as_str()),
            ("RUNFILES_DIR", runfiles.as_str()),
            ("DX_MACOS_APP_ZIP", "_main/pkg/Ledger.zip"),
            ("TMPDIR", tmp.as_str()),
        ],
    )
    .unwrap_or_else(|error| panic!("run: {error}"));
    assert_eq!(ran.status.code(), Some(0), "{}", ran.combined());
    assert_eq!(ran.stdout, "ledger --flag\n");
}
