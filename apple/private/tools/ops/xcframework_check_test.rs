//! Tests the iOS XCFramework slice check.

mod fake;

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

fn loads(platform: &str, minos: &str) -> String {
    format!(
        "Load command 3\n          cmd LC_ID_DYLIB\n      cmdsize 56\n         name @rpath/Share.framework/Share (offset 24)\nLoad command 9\n      cmd LC_BUILD_VERSION\n  cmdsize 32\n platform {platform}\n    minos {minos}\n      sdk 18.5\n"
    )
}

struct Tree(Vec<(String, Vec<u8>)>);

impl Tree {
    fn new() -> Tree {
        let mut tree = Tree(Vec::new());
        tree.file("Share.xcframework/Info.plist", ROOT_PLIST);
        tree.library(DEVICE, "iPhoneOS", &[("arm64", "2")]);
        tree.library(
            SIMULATOR,
            "iPhoneSimulator",
            &[("x86_64", "7"), ("arm64", "7")],
        );
        tree
    }

    fn file(&mut self, path: &str, text: &str) {
        self.0.retain(|(have, _)| have != path);
        self.0.push((path.to_owned(), text.as_bytes().to_vec()));
    }

    fn remove(&mut self, path: &str) {
        self.0.retain(|(have, _)| have != path);
    }

    fn text(&self, path: &str) -> String {
        self.0
            .iter()
            .find(|(have, _)| have == path)
            .map(|(_, bytes)| String::from_utf8(bytes.clone()).unwrap())
            .unwrap_or_default()
    }

    fn library(&mut self, framework: &str, supported: &str, archs: &[(&str, &str)]) {
        self.file(&format!("{framework}/Share"), "mach-o");
        let names: Vec<&str> = archs.iter().map(|(arch, _)| *arch).collect();
        self.file(&format!("{framework}/Share.archs"), &names.join(" "));
        for (arch, platform) in archs {
            self.file(
                &format!("{framework}/Share.{arch}.loads"),
                &loads(platform, "17.0"),
            );
        }
        self.file(
            &format!("{framework}/Info.plist"),
            &format!(
                "CFBundleIdentifier=com.example.share\nCFBundleExecutable=Share\nCFBundlePackageType=FMWK\nCFBundleShortVersionString=1.4\nMinimumOSVersion=17.0\nCFBundleSupportedPlatforms.0={supported}\n"
            ),
        );
        self.file(&format!("{framework}/rates.txt"), "tea 450\n");
        self.file(
            &format!("{framework}/Headers/share_api.h"),
            "int share_total(void);\n",
        );
        self.file(
            &format!("{framework}/Headers/Share.h"),
            "#import <Share/share_api.h>\n",
        );
        self.file(
            &format!("{framework}/Modules/module.modulemap"),
            "framework module Share {\n  umbrella header \"Share.h\"\n  export *\n  module * { export * }\n}\n",
        );
    }

    fn write(&self, path: &Path) {
        let mut writer = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        for (name, bytes) in &self.0 {
            writer.start_file(name.as_str(), options).unwrap();
            writer.write_all(bytes).unwrap();
        }
        writer.finish().unwrap();
    }
}

fn read(path: &str) -> Option<String> {
    std::fs::read_to_string(path).ok()
}

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
    match cmd.args.first().map(String::as_str) {
        Some("lipo") => match read(&format!("{}.archs", cmd.args[2])) {
            Some(archs) => ok(&format!("{archs}\n")),
            None => fail(1, "not a Mach-O"),
        },
        Some("otool") => match read(&format!("{}.{}.loads", cmd.args[4], cmd.args[2])) {
            Some(text) => ok(&text),
            None => fail(1, "no such arch"),
        },
        other => panic!("unexpected {other:?}"),
    }
}

struct Checked {
    outcome: Result<u8, String>,
    report: PathBuf,
    out: PathBuf,
    archive: PathBuf,
    root: dx_test_scratch::TempDir,
}

fn check_archive(write: impl FnOnce(&Path), slices: &str, resources: &str) -> Checked {
    let root = scratch("xcframework-check");
    let out_dir = root.path().join("out");
    std::fs::create_dir_all(&out_dir).unwrap();
    let archive = root.path().join("Share_bundle.xcframework.zip");
    write(&archive);
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
        archive,
        root,
    }
}

fn check_resources(tree: &Tree, slices: &str, resources: &str) -> Checked {
    check_archive(|path| tree.write(path), slices, resources)
}

fn check(tree: &Tree, slices: &str) -> Checked {
    check_resources(tree, slices, "rates.txt")
}

fn rejected(checked: &Checked) -> &str {
    let message = checked.outcome.as_ref().err().map_or("", String::as_str);
    assert!(!checked.report.exists(), "{message}: report was written");
    assert!(
        !checked.out.exists(),
        "{message}: a rejected XCFramework is published"
    );
    let left: Vec<_> = std::fs::read_dir(checked.root.path().join("out"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    assert!(left.is_empty(), "{message}: left {left:?}");
    message
}

fn rejects(tree: &Tree, slices: &str, want: &str) {
    let checked = check(tree, slices);
    assert_eq!(rejected(&checked), want);
}

#[test]
fn the_slice_loads_are_read() {
    let found = parse_slice(&format!("{}{}", loads("7", "16.0"), loads("7", "17.2")));
    assert_eq!(found.platforms, ["7"]);
    assert_eq!(found.minos, "17.2");
    assert_eq!(found.install_name, "@rpath/Share.framework/Share");
    assert_eq!(parse_slice(""), Default::default());
}

#[test]
fn a_truthful_xcframework_records_every_slice_and_is_published() {
    let checked = check(&Tree::new(), SLICES);
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
    assert_eq!(
        std::fs::read(&checked.out).unwrap(),
        std::fs::read(&checked.archive).unwrap()
    );
    let left: Vec<_> = std::fs::read_dir(checked.root.path().join("out"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    let mut left = left;
    left.sort();
    assert_eq!(left, ["Share.xcframework.zip", "Share_xcframework.txt"]);
}

#[test]
fn missing_and_undeclared_slices_fail() {
    let mut tree = Tree::new();
    rejects(
        &tree,
        "device/arm64,simulator/arm64",
        "Share.xcframework carries the undeclared slice simulator/x86_64",
    );

    tree.file(
        "Share.xcframework/Info.plist",
        &ROOT_PLIST.replace("AvailableLibraries.1.SupportedArchitectures.1=x86_64\n", ""),
    );
    rejects(
        &tree,
        SLICES,
        "binary ios-arm64_x86_64-simulator/Share.framework/Share carries x86_64, which library ios-arm64_x86_64-simulator does not list",
    );

    tree.file(&format!("{SIMULATOR}/Share.archs"), "arm64");
    rejects(
        &tree,
        SLICES,
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
    tree.file(
        "Share.xcframework/Info.plist",
        &format!(
            "{ROOT_PLIST}AvailableLibraries.2.LibraryIdentifier=ios-arm64-simulator\nAvailableLibraries.2.LibraryPath=Share.framework\nAvailableLibraries.2.SupportedPlatform=ios\nAvailableLibraries.2.SupportedPlatformVariant=simulator\nAvailableLibraries.2.SupportedArchitectures.0=arm64\n"
        ),
    );
    rejects(
        &tree,
        SLICES,
        "slice simulator/arm64 appears in more than one library",
    );
}

#[test]
fn a_library_for_another_platform_or_variant_fails() {
    let mut tree = Tree::new();
    tree.file(
        "Share.xcframework/Info.plist",
        &ROOT_PLIST.replace(
            "AvailableLibraries.0.SupportedPlatform=ios",
            "AvailableLibraries.0.SupportedPlatform=macos",
        ),
    );
    rejects(&tree, SLICES, "library ios-arm64 is for 'macos', want ios");

    tree.file(
        "Share.xcframework/Info.plist",
        &format!("{ROOT_PLIST}AvailableLibraries.0.SupportedPlatformVariant=maccatalyst\n"),
    );
    rejects(
        &tree,
        SLICES,
        "library ios-arm64 has variant 'maccatalyst', want device or simulator",
    );
}

#[test]
fn a_slice_built_for_the_other_platform_fails() {
    let mut tree = Tree::new();
    tree.file(
        &format!("{SIMULATOR}/Share.arm64.loads"),
        &loads("2", "17.0"),
    );
    rejects(
        &tree,
        SLICES,
        "slice simulator/arm64 declares build platform '2', want 7 for iphonesimulator",
    );
}

#[test]
fn a_slice_above_the_framework_deployment_fails() {
    let mut tree = Tree::new();
    tree.file(&format!("{DEVICE}/Share.arm64.loads"), &loads("2", "18.0"));
    rejects(
        &tree,
        SLICES,
        "slice device/arm64 needs iOS 18.0, above the framework deployment 17.0",
    );
}

#[test]
fn a_missing_module_map_or_public_header_fails() {
    let mut tree = Tree::new();
    tree.remove(&format!("{DEVICE}/Modules/module.modulemap"));
    rejects(
        &tree,
        SLICES,
        "library ios-arm64 has no Share.framework/Modules/module.modulemap",
    );

    tree.file(
        &format!("{DEVICE}/Modules/module.modulemap"),
        "module Other {\n}\n",
    );
    rejects(
        &tree,
        SLICES,
        "ios-arm64/Share.framework/Modules/module.modulemap declares no framework module Share",
    );

    tree.file(
        &format!("{DEVICE}/Modules/module.modulemap"),
        "framework module Share {\n}\n",
    );
    tree.remove(&format!("{SIMULATOR}/Headers/share_api.h"));
    rejects(
        &tree,
        SLICES,
        "library ios-arm64_x86_64-simulator misses public header Headers/share_api.h",
    );
}

#[test]
fn resource_names_with_spaces_and_commas_are_checked_whole() {
    let mut tree = Tree::new();
    for framework in [DEVICE, SIMULATOR] {
        tree.file(&format!("{framework}/Rate Card.txt"), "tea 450\n");
        tree.file(&format!("{framework}/fees, 2.txt"), "fee 2\n");
    }
    let checked = check_resources(&tree, SLICES, "Rate Card.txt\nfees, 2.txt");
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
        &tree,
        SLICES,
        "library ios-arm64_x86_64-simulator misses resource Share.framework/rates.txt",
    );
}

#[test]
fn wrong_bundle_metadata_and_install_names_fail() {
    let mut tree = Tree::new();
    let plist = format!("{DEVICE}/Info.plist");
    let good = tree.text(&plist);
    tree.file(
        &plist,
        &good.replace("com.example.share", "com.example.other"),
    );
    rejects(
        &tree,
        SLICES,
        "ios-arm64/Share.framework has CFBundleIdentifier 'com.example.other', want 'com.example.share'",
    );

    tree.file(
        &plist,
        &good.replace("MinimumOSVersion=17.0", "MinimumOSVersion=18.0"),
    );
    rejects(
        &tree,
        SLICES,
        "ios-arm64/Share.framework has MinimumOSVersion '18.0', want '17.0'",
    );

    tree.file(&plist, &good);
    tree.file(
        &format!("{DEVICE}/Share.arm64.loads"),
        &loads("2", "17.0").replace("@rpath/Share.framework/Share", "@rpath/libShare.dylib"),
    );
    rejects(
        &tree,
        SLICES,
        "slice device/arm64 installs as '@rpath/libShare.dylib', want @rpath/Share.framework/Share",
    );
}

#[test]
fn a_broken_archive_or_binary_fails() {
    let checked = check_archive(
        |path| std::fs::write(path, b"PK\x03\x04share").unwrap(),
        SLICES,
        "rates.txt",
    );
    assert!(
        rejected(&checked).starts_with("Share_bundle.xcframework.zip does not unpack: "),
        "{:?}",
        checked.outcome
    );

    let mut tree = Tree::new();
    tree.remove("Share.xcframework/Info.plist");
    rejects(&tree, SLICES, "Share.xcframework has no Info.plist");

    let mut tree = Tree::new();
    tree.remove(&format!("{DEVICE}/Share.archs"));
    rejects(
        &tree,
        SLICES,
        "binary ios-arm64/Share.framework/Share is not a Mach-O file",
    );
}
