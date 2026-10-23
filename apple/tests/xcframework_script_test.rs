use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use dx_testing::{mkscratch, resolve_runfiles, run, Run};

const FAKE_UNZIP: &str = r#"#!/bin/sh
if [ -n "${FAKE_BAD_ZIP:-}" ]; then
    echo "unzip: cannot find zipfile directory" >&2
    exit 9
fi
cp -R "$FAKE_TREE/." "$4"
if [ -n "${FAKE_ZIP_CLASH:-}" ]; then
    echo "replace $4/Share.xcframework/ios-arm64/Share.framework/Headers/Share.h? [y]es, [n]o, [A]ll, [N]one, [r]ename:  NULL"
    echo "(EOF or read error, treating as \"[N]one\" ...)"
    exit 1
fi
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

const ROOT_PLIST: &str = "XCFrameworkFormatVersion=1.0
AvailableLibraries.0.LibraryIdentifier=ios-arm64
AvailableLibraries.0.LibraryPath=Share.framework
AvailableLibraries.0.SupportedPlatform=ios
AvailableLibraries.0.SupportedArchitectures.0=arm64
AvailableLibraries.1.LibraryIdentifier=ios-arm64_x86_64-simulator
AvailableLibraries.1.LibraryPath=Share.framework
AvailableLibraries.1.SupportedPlatform=ios
AvailableLibraries.1.SupportedPlatformVariant=simulator
AvailableLibraries.1.SupportedArchitectures.0=arm64
AvailableLibraries.1.SupportedArchitectures.1=x86_64
";

const SLICES: &str = "device/arm64,simulator/arm64,simulator/x86_64";

const ZIP_BYTES: &str = "PK\u{3}\u{4}share";

fn loads(platform: &str, minos: &str) -> String {
    format!(
        "Load command 3\n          cmd LC_ID_DYLIB\n      cmdsize 56\n         name @rpath/Share.framework/Share (offset 24)\nLoad command 9\n      cmd LC_BUILD_VERSION\n  cmdsize 32\n platform {platform}\n    minos {minos}\n      sdk 18.5\n"
    )
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

fn library(tree: &Path, identifier: &str, supported: &str, archs: &[(&str, &str)]) -> PathBuf {
    let framework = tree
        .join("Share.xcframework")
        .join(identifier)
        .join("Share.framework");
    write(&framework.join("Share"), "mach-o");
    let names: Vec<&str> = archs.iter().map(|(arch, _)| *arch).collect();
    write(&framework.join("Share.archs"), &names.join(" "));
    for (arch, platform) in archs {
        write(
            &framework.join(format!("Share.{arch}.loads")),
            &loads(platform, "17.0"),
        );
    }
    write(
        &framework.join("Info.plist"),
        &format!("CFBundleIdentifier=com.example.share\nCFBundleExecutable=Share\nCFBundlePackageType=FMWK\nCFBundleShortVersionString=1.4\nMinimumOSVersion=17.0\nCFBundleSupportedPlatforms.0={supported}\n"),
    );
    write(&framework.join("rates.txt"), "tea 450\n");
    write(
        &framework.join("Headers/share_api.h"),
        "int share_total(void);\n",
    );
    write(
        &framework.join("Headers/Share.h"),
        "#import <Share/share_api.h>\n",
    );
    write(
        &framework.join("Modules/module.modulemap"),
        "framework module Share {\n  umbrella header \"Share.h\"\n  export *\n  module * { export * }\n}\n",
    );
    framework
}

struct Case {
    root: PathBuf,
    tree: PathBuf,
    device: PathBuf,
    simulator: PathBuf,
}

impl Case {
    fn new(prefix: &str) -> Case {
        let root = mkscratch(prefix).unwrap_or_else(|error| panic!("scratch: {error}"));
        let bin = root.join("bin");
        executable(&bin.join("unzip"), FAKE_UNZIP);
        executable(&bin.join("plutil"), FAKE_PLUTIL);
        executable(&bin.join("xcrun"), FAKE_XCRUN);
        let tree = root.join("tree");
        write(&tree.join("Share.xcframework/Info.plist"), ROOT_PLIST);
        let device = library(&tree, "ios-arm64", "iPhoneOS", &[("arm64", "2")]);
        let simulator = library(
            &tree,
            "ios-arm64_x86_64-simulator",
            "iPhoneSimulator",
            &[("x86_64", "7"), ("arm64", "7")],
        );
        write(&root.join("Share_bundle.xcframework.zip"), ZIP_BYTES);
        Case {
            root,
            tree,
            device,
            simulator,
        }
    }

    fn report(&self) -> PathBuf {
        self.root.join("out/Share_xcframework.txt")
    }

    fn out(&self) -> PathBuf {
        self.root.join("out/Share.xcframework.zip")
    }

    fn check(&self, slices: &str, envs: &[(&str, &str)]) -> Run {
        std::fs::create_dir_all(self.root.join("out"))
            .unwrap_or_else(|error| panic!("mkdir: {error}"));
        let path = format!(
            "{}:{}",
            self.root.join("bin").display(),
            std::env::var("PATH").unwrap_or_default()
        );
        let report = self.report().to_string_lossy().into_owned();
        let out = self.out().to_string_lossy().into_owned();
        let archive = self
            .root
            .join("Share_bundle.xcframework.zip")
            .to_string_lossy()
            .into_owned();
        let tree = self.tree.to_string_lossy().into_owned();
        let mut all = vec![("PATH", path.as_str()), ("FAKE_TREE", tree.as_str())];
        all.extend_from_slice(envs);
        let script = resolve_runfiles(
            &std::env::var("DX_XCFRAMEWORK_CHECK")
                .unwrap_or_else(|_| panic!("DX_XCFRAMEWORK_CHECK must name a script")),
        );
        run(
            &script,
            &[
                report.as_str(),
                out.as_str(),
                archive.as_str(),
                "Share",
                "com.example.share",
                "1.4",
                "17.0",
                "16.4.0.16F6",
                slices,
                "rates.txt",
                "share_api.h",
            ],
            &all,
        )
        .unwrap_or_else(|error| panic!("run: {error}"))
    }

    fn rejects(&self, slices: &str, envs: &[(&str, &str)], want: &str) {
        let ran = self.check(slices, envs);
        assert_eq!(ran.status.code(), Some(1), "{}", ran.combined());
        assert_eq!(ran.stderr, format!("apple_ios_framework: {want}\n"));
        assert!(
            !self.report().exists(),
            "{want}: a rejected XCFramework leaves no report"
        );
        assert!(
            !self.out().exists(),
            "{want}: a rejected XCFramework is not published"
        );
        assert!(
            !self
                .root
                .join("out/Share_xcframework.txt.unpacked")
                .exists(),
            "{want}: the unpacked copy outlived the check"
        );
    }
}

#[test]
fn a_truthful_xcframework_records_every_slice_and_is_published() {
    let case = Case::new("xcframework-ok");
    let ran = case.check(SLICES, &[]);
    assert_eq!(ran.status.code(), Some(0), "{}", ran.combined());
    assert_eq!(
        std::fs::read_to_string(case.report()).unwrap_or_default(),
        "xcframework=Share
bundle_id=com.example.share
version=1.4
deployment=17.0
xcode=16.4.0.16F6
resources=rates.txt
slice=device/arm64 library=ios-arm64 platform=iphoneos minos=17.0 install_name=@rpath/Share.framework/Share
slice=simulator/arm64 library=ios-arm64_x86_64-simulator platform=iphonesimulator minos=17.0 install_name=@rpath/Share.framework/Share
slice=simulator/x86_64 library=ios-arm64_x86_64-simulator platform=iphonesimulator minos=17.0 install_name=@rpath/Share.framework/Share
"
    );
    assert_eq!(
        std::fs::read_to_string(case.out()).unwrap_or_default(),
        ZIP_BYTES
    );
    assert!(!case
        .root
        .join("out/Share_xcframework.txt.unpacked")
        .exists());
}

#[test]
fn missing_and_undeclared_slices_fail() {
    let case = Case::new("xcframework-slices");
    case.rejects(
        "device/arm64,simulator/arm64",
        &[],
        "Share.xcframework carries the undeclared slice simulator/x86_64",
    );

    let plist = case.tree.join("Share.xcframework/Info.plist");
    write(
        &plist,
        &ROOT_PLIST.replace("AvailableLibraries.1.SupportedArchitectures.1=x86_64\n", ""),
    );
    case.rejects(
        SLICES,
        &[],
        "binary ios-arm64_x86_64-simulator/Share.framework/Share carries x86_64, which library ios-arm64_x86_64-simulator does not list",
    );

    write(&case.simulator.join("Share.archs"), "arm64");
    case.rejects(
        SLICES,
        &[],
        "Share.xcframework misses the declared slice simulator/x86_64",
    );
}

#[test]
fn a_slice_listed_twice_fails() {
    let case = Case::new("xcframework-duplicate");
    library(
        &case.tree,
        "ios-arm64-simulator",
        "iPhoneSimulator",
        &[("arm64", "7")],
    );
    write(
        &case.tree.join("Share.xcframework/Info.plist"),
        &format!("{ROOT_PLIST}AvailableLibraries.2.LibraryIdentifier=ios-arm64-simulator\nAvailableLibraries.2.LibraryPath=Share.framework\nAvailableLibraries.2.SupportedPlatform=ios\nAvailableLibraries.2.SupportedPlatformVariant=simulator\nAvailableLibraries.2.SupportedArchitectures.0=arm64\n"),
    );
    case.rejects(
        SLICES,
        &[],
        "slice simulator/arm64 appears in more than one library",
    );
}

#[test]
fn a_slice_built_for_the_other_platform_fails() {
    let case = Case::new("xcframework-platform");
    write(
        &case.simulator.join("Share.arm64.loads"),
        &loads("2", "17.0"),
    );
    case.rejects(
        SLICES,
        &[],
        "slice simulator/arm64 declares build platform '2', want 7 for iphonesimulator",
    );
}

#[test]
fn a_slice_above_the_framework_deployment_fails() {
    let case = Case::new("xcframework-minos");
    write(&case.device.join("Share.arm64.loads"), &loads("2", "18.0"));
    case.rejects(
        SLICES,
        &[],
        "slice device/arm64 needs iOS 18.0, above the framework deployment 17.0",
    );
}

#[test]
fn a_missing_module_map_or_public_header_fails() {
    let case = Case::new("xcframework-module");
    std::fs::remove_file(case.device.join("Modules/module.modulemap"))
        .unwrap_or_else(|error| panic!("rm: {error}"));
    case.rejects(
        SLICES,
        &[],
        "library ios-arm64 has no Share.framework/Modules/module.modulemap",
    );

    write(
        &case.device.join("Modules/module.modulemap"),
        "module Other {\n}\n",
    );
    case.rejects(
        SLICES,
        &[],
        "ios-arm64/Share.framework/Modules/module.modulemap declares no framework module Share",
    );

    write(
        &case.device.join("Modules/module.modulemap"),
        "framework module Share {\n}\n",
    );
    std::fs::remove_file(case.simulator.join("Headers/share_api.h"))
        .unwrap_or_else(|error| panic!("rm: {error}"));
    case.rejects(
        SLICES,
        &[],
        "library ios-arm64_x86_64-simulator misses public header Headers/share_api.h",
    );
}

#[test]
fn a_missing_resource_fails() {
    let case = Case::new("xcframework-resource");
    std::fs::remove_file(case.simulator.join("rates.txt"))
        .unwrap_or_else(|error| panic!("rm: {error}"));
    case.rejects(
        SLICES,
        &[],
        "library ios-arm64_x86_64-simulator misses resource Share.framework/rates.txt",
    );
}

#[test]
fn wrong_bundle_metadata_and_install_names_fail() {
    let case = Case::new("xcframework-metadata");
    let plist = case.device.join("Info.plist");
    let good = std::fs::read_to_string(&plist).unwrap_or_default();
    write(
        &plist,
        &good.replace("com.example.share", "com.example.other"),
    );
    case.rejects(
        SLICES,
        &[],
        "ios-arm64/Share.framework has CFBundleIdentifier 'com.example.other', want 'com.example.share'",
    );

    write(
        &plist,
        &good.replace("MinimumOSVersion=17.0", "MinimumOSVersion=18.0"),
    );
    case.rejects(
        SLICES,
        &[],
        "ios-arm64/Share.framework has MinimumOSVersion '18.0', want '17.0'",
    );

    write(&plist, &good);
    write(
        &case.device.join("Share.arm64.loads"),
        &loads("2", "17.0").replace("@rpath/Share.framework/Share", "@rpath/libShare.dylib"),
    );
    case.rejects(
        SLICES,
        &[],
        "slice device/arm64 installs as '@rpath/libShare.dylib', want @rpath/Share.framework/Share",
    );
}

#[test]
fn a_broken_archive_fails() {
    let case = Case::new("xcframework-zip");
    case.rejects(
        SLICES,
        &[("FAKE_BAD_ZIP", "1")],
        "Share_bundle.xcframework.zip does not unpack: unzip: cannot find zipfile directory",
    );

    case.rejects(
        SLICES,
        &[("FAKE_ZIP_CLASH", "1")],
        "Share_bundle.xcframework.zip does not unpack: (EOF or read error, treating as \"[N]one\" ...)",
    );

    std::fs::remove_file(case.device.join("Share.archs"))
        .unwrap_or_else(|error| panic!("rm: {error}"));
    case.rejects(
        SLICES,
        &[],
        "binary ios-arm64/Share.framework/Share is not a Mach-O file",
    );
}
