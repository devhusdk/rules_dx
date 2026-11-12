//! Tests the iOS XCFramework slice check.

mod fake;

use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};

use dx_apple_ops::util::{Cmd, Output};
use dx_apple_ops::xcframework_check::{parse_slice, run};
use dx_apple_ops::xcode::{PLUTIL, Tools, XCRUN};
use dx_test_scratch::scratch;
use fake::{Fake, args, fail, ok};

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

const DEVICE: &str = "Share.xcframework/ios-arm64/Share.framework";
const SIMULATOR: &str = "Share.xcframework/ios-arm64_x86_64-simulator/Share.framework";
const INDEX: &str = "Share.xcframework/Info.plist";

fn loads(platform: &str, minos: &str) -> String {
    format!(
        "Load command 3\n          cmd LC_ID_DYLIB\n      cmdsize 56\n         name @rpath/Share.framework/Share (offset 24)\nLoad command 9\n      cmd LC_BUILD_VERSION\n  cmdsize 32\n platform {platform}\n    minos {minos}\n      sdk 18.5\n"
    )
}

/// The archive contents: path to file text.
struct Tree(BTreeMap<String, String>);

impl Tree {
    fn new() -> Tree {
        let mut tree = Tree(BTreeMap::new());
        tree.set(INDEX, ROOT_PLIST);
        tree.library(DEVICE, "iPhoneOS", &[("arm64", "2")]);
        tree.library(
            SIMULATOR,
            "iPhoneSimulator",
            &[("x86_64", "7"), ("arm64", "7")],
        );
        tree
    }

    fn set(&mut self, path: &str, text: &str) {
        self.0.insert(path.to_owned(), text.to_owned());
    }

    fn remove(&mut self, path: &str) {
        assert!(self.0.remove(path).is_some(), "{path} is not in the tree");
    }

    fn library(&mut self, framework: &str, supported: &str, archs: &[(&str, &str)]) {
        self.set(&format!("{framework}/Share"), "mach-o");
        let names: Vec<&str> = archs.iter().map(|(arch, _)| *arch).collect();
        self.set(&format!("{framework}/Share.archs"), &names.join(" "));
        for (arch, platform) in archs {
            self.set(
                &format!("{framework}/Share.{arch}.loads"),
                &loads(platform, "17.0"),
            );
        }
        self.set(
            &format!("{framework}/Info.plist"),
            &format!(
                "CFBundleIdentifier=com.example.share\nCFBundleExecutable=Share\nCFBundlePackageType=FMWK\nCFBundleShortVersionString=1.4\nMinimumOSVersion=17.0\nCFBundleSupportedPlatforms.0={supported}\n"
            ),
        );
        self.set(&format!("{framework}/rates.txt"), "tea 450\n");
        self.set(
            &format!("{framework}/Headers/share_api.h"),
            "int share_total(void);\n",
        );
        self.set(
            &format!("{framework}/Headers/Share.h"),
            "#import <Share/share_api.h>\n",
        );
        self.set(
            &format!("{framework}/Modules/module.modulemap"),
            "framework module Share {\n  umbrella header \"Share.h\"\n  export *\n  module * { export * }\n}\n",
        );
    }

    fn write(&self, path: &Path) {
        let mut writer = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        for (name, text) in &self.0 {
            writer.start_file(name.as_str(), options).unwrap();
            writer.write_all(text.as_bytes()).unwrap();
        }
        writer.finish().unwrap();
    }
}

fn read(path: &str) -> Option<String> {
    std::fs::read_to_string(path).ok()
}

/// Answers plutil from `key=value` lines and xcrun from the `.archs` and `.loads` files next to a binary.
fn tool(cmd: &Cmd) -> Output {
    if cmd.program == PLUTIL {
        let key = &cmd.args[1];
        let text = read(&cmd.args[5]).unwrap_or_default();
        return match text
            .lines()
            .find_map(|line| line.strip_prefix(&format!("{key}=")))
        {
            Some(value) => ok(&format!("{value}\n")),
            None => fail(1, "No value at that key path"),
        };
    }
    assert_eq!(cmd.program, XCRUN);
    let binary = cmd.args.last().unwrap();
    match cmd.args.first().map(String::as_str) {
        Some("lipo") => read(&format!("{binary}.archs")).map_or_else(
            || fail(1, "not a Mach-O"),
            |archs| ok(&format!("{archs}\n")),
        ),
        Some("otool") => read(&format!("{binary}.{}.loads", cmd.args[2]))
            .map_or_else(|| fail(1, "no such arch"), |text| ok(&text)),
        other => panic!("unexpected {other:?}"),
    }
}

struct Checked {
    outcome: Result<u8, String>,
    report: PathBuf,
    out: PathBuf,
    archive: Vec<u8>,
    root: dx_test_scratch::TempDir,
}

fn check_with(tree: &Tree, slices: &str, resources: &str) -> Checked {
    let root = scratch("xcframework-check");
    let out_dir = root.path().join("out");
    std::fs::create_dir_all(&out_dir).unwrap();
    let archive = root.path().join("Share_bundle.xcframework.zip");
    tree.write(&archive);
    let report = out_dir.join("Share_xcframework.txt");
    let out = out_dir.join("Share.xcframework.zip");
    let fake = Fake::new(tool);
    let tools = Tools {
        developer_dir: None,
        runner: &fake,
    };
    let report_text = report.to_string_lossy().into_owned();
    let out_text = out.to_string_lossy().into_owned();
    let archive_text = archive.to_string_lossy().into_owned();
    let argv = args(&[
        &report_text,
        &out_text,
        &archive_text,
        "Share",
        "com.example.share",
        "1.4",
        "17.0",
        "16.4.0.16F6",
        slices,
        resources,
        "share_api.h",
    ]);
    let outcome = run(&tools, &argv).map_err(|failure| failure.message);
    Checked {
        outcome,
        report,
        out,
        archive: std::fs::read(&archive).unwrap(),
        root,
    }
}

fn check(tree: &Tree) -> Checked {
    check_with(tree, SLICES, "rates.txt")
}

fn rejects(checked: &Checked, want: &str) {
    assert_eq!(
        checked.outcome.as_ref().err().map(String::as_str),
        Some(want)
    );
    let left: Vec<_> = std::fs::read_dir(checked.root.path().join("out"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    assert!(left.is_empty(), "{want}: left {left:?}");
}

#[test]
fn a_truthful_xcframework_records_every_slice_and_is_published() {
    let checked = check(&Tree::new());
    assert_eq!(checked.outcome, Ok(0));
    assert_eq!(
        std::fs::read_to_string(&checked.report).unwrap(),
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
    assert_eq!(std::fs::read(&checked.out).unwrap(), checked.archive);
    let left: Vec<_> = std::fs::read_dir(checked.root.path().join("out"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    assert_eq!(left, ["Share.xcframework.zip", "Share_xcframework.txt"]);
}

#[test]
fn missing_and_undeclared_slices_fail() {
    let mut tree = Tree::new();
    rejects(
        &check_with(&tree, "device/arm64,simulator/arm64", "rates.txt"),
        "Share.xcframework carries the undeclared slice simulator/x86_64",
    );

    tree.set(
        INDEX,
        &ROOT_PLIST.replace("AvailableLibraries.1.SupportedArchitectures.1=x86_64\n", ""),
    );
    rejects(
        &check(&tree),
        "binary ios-arm64_x86_64-simulator/Share.framework/Share carries x86_64, which library ios-arm64_x86_64-simulator does not list",
    );

    tree.set(&format!("{SIMULATOR}/Share.archs"), "arm64");
    rejects(
        &check(&tree),
        "Share.xcframework misses the declared slice simulator/x86_64",
    );
}

#[test]
fn a_slice_listed_twice_fails() {
    let mut tree = Tree::new();
    tree.library(
        "Share.xcframework/ios-arm64-simulator/Share.framework",
        "iPhoneSimulator",
        &[("arm64", "7")],
    );
    tree.set(
        INDEX,
        &format!(
            "{ROOT_PLIST}AvailableLibraries.2.LibraryIdentifier=ios-arm64-simulator\nAvailableLibraries.2.LibraryPath=Share.framework\nAvailableLibraries.2.SupportedPlatform=ios\nAvailableLibraries.2.SupportedPlatformVariant=simulator\nAvailableLibraries.2.SupportedArchitectures.0=arm64\n"
        ),
    );
    rejects(
        &check(&tree),
        "slice simulator/arm64 appears in more than one library",
    );
}

#[test]
fn a_slice_built_for_the_other_platform_fails() {
    let mut tree = Tree::new();
    tree.set(
        &format!("{SIMULATOR}/Share.arm64.loads"),
        &loads("2", "17.0"),
    );
    rejects(
        &check(&tree),
        "slice simulator/arm64 declares build platform '2', want 7 for iphonesimulator",
    );

    tree.set(
        &format!("{SIMULATOR}/Share.arm64.loads"),
        &format!("{}{}", loads("7", "17.0"), loads("2", "17.0")),
    );
    rejects(
        &check(&tree),
        "slice simulator/arm64 declares build platform '2 7', want 7 for iphonesimulator",
    );
}

#[test]
fn a_slice_above_the_framework_deployment_fails() {
    let mut tree = Tree::new();
    tree.set(&format!("{DEVICE}/Share.arm64.loads"), &loads("2", "18.0"));
    rejects(
        &check(&tree),
        "slice device/arm64 needs iOS 18.0, above the framework deployment 17.0",
    );

    tree.set(
        &format!("{DEVICE}/Share.arm64.loads"),
        &loads("2", "17.0").replace("    minos 17.0\n", ""),
    );
    rejects(
        &check(&tree),
        "slice device/arm64 declares no minimum iOS version",
    );
}

#[test]
fn a_missing_module_map_or_public_header_fails() {
    let mut tree = Tree::new();
    tree.remove(&format!("{DEVICE}/Modules/module.modulemap"));
    rejects(
        &check(&tree),
        "library ios-arm64 has no Share.framework/Modules/module.modulemap",
    );

    tree.set(
        &format!("{DEVICE}/Modules/module.modulemap"),
        "module Other {\n}\n",
    );
    rejects(
        &check(&tree),
        "ios-arm64/Share.framework/Modules/module.modulemap declares no framework module Share",
    );

    tree.set(
        &format!("{DEVICE}/Modules/module.modulemap"),
        "framework module Share {\n}\n",
    );
    tree.remove(&format!("{SIMULATOR}/Headers/share_api.h"));
    rejects(
        &check(&tree),
        "library ios-arm64_x86_64-simulator misses public header Headers/share_api.h",
    );
}

#[test]
fn resource_names_with_spaces_and_commas_are_checked_whole() {
    let mut tree = Tree::new();
    for framework in [DEVICE, SIMULATOR] {
        tree.set(&format!("{framework}/Rate Card.txt"), "tea 450\n");
        tree.set(&format!("{framework}/fees, 2.txt"), "fee 2\n");
    }
    let checked = check_with(&tree, SLICES, "Rate Card.txt\nfees, 2.txt");
    assert_eq!(checked.outcome, Ok(0));
    assert!(
        std::fs::read_to_string(&checked.report)
            .unwrap()
            .contains("\nresources=Rate Card.txt,fees, 2.txt\n")
    );
}

#[test]
fn a_missing_resource_fails() {
    let mut tree = Tree::new();
    tree.remove(&format!("{SIMULATOR}/rates.txt"));
    rejects(
        &check(&tree),
        "library ios-arm64_x86_64-simulator misses resource Share.framework/rates.txt",
    );
}

#[test]
fn wrong_bundle_metadata_and_install_names_fail() {
    let plist = format!("{DEVICE}/Info.plist");
    let good = Tree::new().0[&plist].clone();
    let mut tree = Tree::new();
    tree.set(
        &plist,
        &good.replace("com.example.share", "com.example.other"),
    );
    rejects(
        &check(&tree),
        "ios-arm64/Share.framework has CFBundleIdentifier 'com.example.other', want 'com.example.share'",
    );

    tree.set(
        &plist,
        &good.replace("MinimumOSVersion=17.0", "MinimumOSVersion=18.0"),
    );
    rejects(
        &check(&tree),
        "ios-arm64/Share.framework has MinimumOSVersion '18.0', want '17.0'",
    );

    tree.set(&plist, &good.replace("CFBundlePackageType=FMWK\n", ""));
    rejects(
        &check(&tree),
        "ios-arm64/Share.framework has CFBundlePackageType '', want 'FMWK'",
    );

    tree.set(&plist, &good);
    tree.set(
        &format!("{DEVICE}/Share.arm64.loads"),
        &loads("2", "17.0").replace("@rpath/Share.framework/Share", "@rpath/libShare.dylib"),
    );
    rejects(
        &check(&tree),
        "slice device/arm64 installs as '@rpath/libShare.dylib', want @rpath/Share.framework/Share",
    );
}

#[test]
fn a_wrong_library_index_fails() {
    let cases = [
        (
            ROOT_PLIST.replace(
                "AvailableLibraries.0.SupportedPlatform=ios",
                "AvailableLibraries.0.SupportedPlatform=macos",
            ),
            "library ios-arm64 is for 'macos', want ios",
        ),
        (
            ROOT_PLIST.replace("PlatformVariant=simulator", "PlatformVariant=maccatalyst"),
            "library ios-arm64_x86_64-simulator has variant 'maccatalyst', want device or simulator",
        ),
        (
            ROOT_PLIST.replace(
                "AvailableLibraries.0.LibraryPath=Share.framework",
                "AvailableLibraries.0.LibraryPath=libShare.a",
            ),
            "library ios-arm64 has path 'libShare.a', want Share.framework",
        ),
        (
            ROOT_PLIST.replace("AvailableLibraries.0.SupportedArchitectures.0=arm64\n", ""),
            "library ios-arm64 lists no architectures",
        ),
        (
            "XCFrameworkFormatVersion=1.0\n".to_owned(),
            "Share.xcframework lists no libraries",
        ),
    ];
    for (index, want) in cases {
        let mut tree = Tree::new();
        tree.set(INDEX, &index);
        rejects(&check(&tree), want);
    }
}

#[test]
fn a_broken_archive_fails() {
    let mut tree = Tree::new();
    tree.remove(INDEX);
    rejects(&check(&tree), "Share.xcframework has no Info.plist");

    let mut tree = Tree::new();
    tree.remove(&format!("{DEVICE}/Share"));
    rejects(
        &check(&tree),
        "library ios-arm64 has no binary Share.framework/Share",
    );

    let mut tree = Tree::new();
    tree.remove(&format!("{DEVICE}/Info.plist"));
    rejects(
        &check(&tree),
        "library ios-arm64 has no Share.framework/Info.plist",
    );

    let mut tree = Tree::new();
    tree.remove(&format!("{DEVICE}/Share.archs"));
    rejects(
        &check(&tree),
        "binary ios-arm64/Share.framework/Share is not a Mach-O file",
    );

    let root = scratch("xcframework-not-zip");
    let out_dir = root.path().join("out");
    std::fs::create_dir_all(&out_dir).unwrap();
    let archive = root.path().join("Share_bundle.xcframework.zip");
    std::fs::write(&archive, "PK\u{3}\u{4}share").unwrap();
    let fake = Fake::new(tool);
    let tools = Tools {
        developer_dir: None,
        runner: &fake,
    };
    let report = out_dir.join("Share_xcframework.txt");
    let report_text = report.to_string_lossy().into_owned();
    let out_text = out_dir
        .join("Share.xcframework.zip")
        .to_string_lossy()
        .into_owned();
    let archive_text = archive.to_string_lossy().into_owned();
    let argv = args(&[
        &report_text,
        &out_text,
        &archive_text,
        "Share",
        "com.example.share",
        "1.4",
        "17.0",
        "16.4.0.16F6",
        SLICES,
        "",
    ]);
    let message = run(&tools, &argv).unwrap_err().message;
    assert!(
        message.starts_with("Share_bundle.xcframework.zip does not unpack: "),
        "{message}"
    );
    let left: Vec<_> = std::fs::read_dir(&out_dir).unwrap().collect();
    assert!(left.is_empty(), "left {left:?}");
    assert!(fake.log().is_empty());
}

#[test]
fn a_short_command_line_is_a_usage_error() {
    let fake = Fake::new(tool);
    let tools = Tools {
        developer_dir: None,
        runner: &fake,
    };
    let failure = run(&tools, &args(&["/nowhere/report.txt"])).unwrap_err();
    assert_eq!(failure.code, 2);
    assert!(
        failure
            .message
            .starts_with("usage: apple_ops xcframework-check ")
    );
}

#[test]
fn otool_text_parses_into_platforms_minos_and_install_name() {
    let text = format!(
        "{}Load command 4\n          cmd LC_LOAD_DYLIB\n         name /usr/lib/libSystem.B.dylib (offset 24)\nLoad command 10\n      cmd LC_BUILD_VERSION\n platform 7\n    minos 16.2\n",
        loads("7", "17.0")
    );
    let slice = parse_slice(&text);
    assert_eq!(slice.platforms, ["7"]);
    assert_eq!(slice.minos, "17.0");
    assert_eq!(slice.install_name, "@rpath/Share.framework/Share");
}
