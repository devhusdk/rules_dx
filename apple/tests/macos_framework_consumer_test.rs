mod consumer;
mod macos;

use std::path::Path;
use std::process::Command;

use consumer::{consumer, fails_with, read, write};
use dx_nested::Case;
use dx_testing::{mkscratch, resolve_runfiles, run, Run};
use macos::{
    bazel, installed_xcode, keep, outputs, passed, passes, runtime_for, tool, wait_for, Simulator,
};

const PRODUCER_EXTRA: &str = r#"
bazel_dep(name = "rules_rust", version = "0.75.0")

apple_cc_configure = use_extension("@xcode_support//crosstool:setup.bzl", "apple_cc_configure_extension")
use_repo(apple_cc_configure, "local_config_apple_cc_toolchains")

register_toolchains("@local_config_apple_cc_toolchains//:all")

rust = use_extension("@rules_rust//rust:extensions.bzl", "rust")
rust.toolchain(
    edition = "2021",
    extra_target_triples = [
        "aarch64-apple-ios",
        "aarch64-apple-ios-sim",
        "x86_64-apple-ios",
    ],
    versions = ["1.98.1"],
)
use_repo(rust, "rust_toolchains")

register_toolchains("@rust_toolchains//:all")
"#;

const CONSUMER_EXTRA: &str = r#"
bazel_dep(name = "rules_apple", version = "5.2.0", repo_name = "apple_rules")

apple_cc_configure = use_extension("@xcode_support//crosstool:setup.bzl", "apple_cc_configure_extension")
use_repo(apple_cc_configure, "local_config_apple_cc_toolchains")

register_toolchains("@local_config_apple_cc_toolchains//:all")
"#;

const CONSUMER_BAZELRC: &str = "build --ios_multi_cpus=sim_arm64\n";

const SLICES: &str = r#"[
        "device/arm64",
        "simulator/arm64",
        "simulator/x86_64",
    ]"#;

const SHARE_BUILD: &str = r#"load("@dx//apple/ios:app.bzl", "apple_ios_framework")
load("@rules_rust//rust:defs.bzl", "rust_static_library")

rust_static_library(
    name = "share_rust",
    srcs = ["share.rs"],
    edition = "2021",
)

apple_ios_framework(
    name = "share",
    bundle_id = "com.example.share",
    bundle_name = "Share",
    deployment = "17.0",
    hdrs = ["share_api.h"],
    library = ":share_rust",
    resources = ["rates.txt"],
    slices = SLICES,
    version = "1.4",
    xcode_version = "XCODE",
)

apple_ios_framework(
    name = "newer",
    bundle_id = "com.example.share",
    bundle_name = "Share",
    deployment = "18.0",
    hdrs = ["share_api.h"],
    library = ":share_rust",
    resources = ["rates.txt"],
    slices = [
        "device/arm64",
        "simulator/arm64",
    ],
    version = "1.4",
    xcode_version = "XCODE",
)

apple_ios_framework(
    name = "partial",
    bundle_id = "com.example.share",
    bundle_name = "Share",
    deployment = "17.0",
    hdrs = ["share_api.h"],
    library = ":share_rust",
    resources = ["rates.txt"],
    slices = [
        "device/arm64",
        "simulator/x86_64",
    ],
    version = "1.4",
    xcode_version = "XCODE",
)
"#;

const SHARE_RS: &str = r#"use std::ffi::{c_char, CStr};

#[no_mangle]
pub extern "C" fn share_total(text: *const c_char) -> i64 {
    if text.is_null() {
        return -1;
    }
    let Ok(text) = unsafe { CStr::from_ptr(text) }.to_str() else {
        return -1;
    };
    let mut total = 0;
    for line in text.lines().map(str::trim).filter(|line| !line.is_empty()) {
        let Some((_, cents)) = line.rsplit_once(' ') else {
            return -1;
        };
        let Ok(cents) = cents.parse::<i64>() else {
            return -1;
        };
        total += cents;
    }
    total
}

#[no_mangle]
pub extern "C" fn share_platform() -> *const c_char {
    if cfg!(all(target_os = "ios", target_abi = "sim")) {
        c"iphonesimulator".as_ptr()
    } else if cfg!(all(target_os = "ios", target_arch = "x86_64")) {
        c"iphonesimulator".as_ptr()
    } else if cfg!(target_os = "ios") {
        c"iphoneos".as_ptr()
    } else {
        c"other".as_ptr()
    }
}
"#;

const SHARE_H: &str = r#"#pragma once

#include <stdint.h>

int64_t share_total(const char *text);
const char *share_platform(void);
"#;

const RATES: &str = "tea 450\nbread 325\njam 600\n";

const BAD_BUILD: &str = r#"load("@dx//apple/ios:app.bzl", "apple_ios_framework")
load("@rules_cc//cc:defs.bzl", "cc_import")

config_setting(
    name = "simulator",
    constraint_values = ["@xcode_support//constraints:simulator"],
)

cc_import(
    name = "device_lib",
    static_library = "libdevice.a",
)

cc_import(
    name = "marker_lib",
    static_library = select({
        ":simulator": "libsimulator.a",
        "//conditions:default": "libdevice.a",
    }),
)

cc_import(
    name = "newer_lib",
    static_library = select({
        ":simulator": "libsimulator18.a",
        "//conditions:default": "libdevice18.a",
    }),
)

apple_ios_framework(
    name = "device_slice",
    bundle_id = "com.example.share",
    bundle_name = "Share",
    deployment = "17.0",
    hdrs = ["marker.h"],
    library = ":device_lib",
    slices = [
        "device/arm64",
        "simulator/arm64",
    ],
    version = "1.0",
)

apple_ios_framework(
    name = "newer_archive",
    bundle_id = "com.example.share",
    bundle_name = "Share",
    deployment = "17.0",
    hdrs = ["marker.h"],
    library = ":newer_lib",
    slices = [
        "device/arm64",
        "simulator/arm64",
    ],
    version = "1.0",
)

apple_ios_framework(
    name = "duplicate_slices",
    bundle_id = "com.example.share",
    bundle_name = "Share",
    deployment = "17.0",
    hdrs = ["marker.h"],
    library = ":device_lib",
    slices = [
        "device/arm64",
        "simulator/arm64",
        "simulator/arm64",
    ],
    version = "1.0",
)
"#;

const MARKER_C: &str = "int share_marker(void) { return 7; }\n";

const MARKER_H: &str = "int share_marker(void);\n";

const TILL_BUILD: &str = r#"load("@apple_rules//apple:apple.bzl", "apple_dynamic_xcframework_import")
load("@dx//apple/ios:app.bzl", "apple_ios_app", "apple_ios_app_test")
load("@dx//apple/ios:simulator.bzl", "apple_simulator_boot", "apple_simulator_install", "apple_simulator_launch")
load("@rules_cc//cc:objc_library.bzl", "objc_library")

apple_dynamic_xcframework_import(
    name = "share",
    xcframework_imports = glob(["Share.xcframework/**"]),
)

objc_library(
    name = "till_headers",
    hdrs = ["TillAppDelegate.h"],
)

apple_ios_app(
    name = "till_app",
    srcs = [
        "TillAppDelegate.m",
        "main.m",
    ],
    arch = "arm64",
    bundle_id = "com.example.till",
    deployment = "17.0",
    environment = "simulator",
    frameworks = [":share"],
    version = "2.0",
    xcode_version = "XCODE",
    deps = [":till_headers"],
)

apple_ios_app_test(
    name = "till_app_test",
    size = "large",
    srcs = ["TillAppTests.m"],
    app = ":till_app",
    deployment = "17.0",
    device_type = "iPhone 16",
    os_version = "RUNTIME",
    deps = [
        ":share",
        ":till_headers",
    ],
)

apple_simulator_boot(
    name = "boot",
    simulator = "SIMULATOR",
)

apple_simulator_install(
    name = "install",
    app = ":till_app",
    simulator = "SIMULATOR",
)

apple_simulator_launch(
    name = "launch",
    bundle_id = "com.example.till",
    simulator = "SIMULATOR",
)
"#;

const DELEGATE_H: &str = r#"#import <UIKit/UIKit.h>

@interface TillAppDelegate : UIResponder <UIApplicationDelegate>
@property(nonatomic, strong) UIWindow *window;
- (int64_t)loadRates;
- (NSURL *)stateURL;
@end
"#;

const DELEGATE_M: &str = r#"#import "till/TillAppDelegate.h"

#import <Share/share_api.h>

@implementation TillAppDelegate

- (int64_t)loadRates {
  NSBundle *share = [NSBundle bundleWithIdentifier:@"com.example.share"];
  NSString *path = [share pathForResource:@"rates" ofType:@"txt"];
  if (path == nil) {
    return -2;
  }
  NSString *text = [NSString stringWithContentsOfFile:path encoding:NSUTF8StringEncoding error:NULL];
  if (text == nil) {
    return -3;
  }
  return share_total(text.UTF8String);
}

- (NSURL *)stateURL {
  NSURL *documents = [[NSFileManager defaultManager] URLsForDirectory:NSDocumentDirectory
                                                            inDomains:NSUserDomainMask]
                         .firstObject;
  return [documents URLByAppendingPathComponent:@"till-state.txt"];
}

- (BOOL)application:(UIApplication *)application
    didFinishLaunchingWithOptions:(NSDictionary *)launchOptions {
  UIViewController *root = [[UIViewController alloc] init];
  self.window = [[UIWindow alloc] initWithFrame:[[UIScreen mainScreen] bounds]];
  self.window.rootViewController = root;
  [self.window makeKeyAndVisible];
  NSString *state = [NSString stringWithFormat:@"platform=%s\ntotal=%lld\n", share_platform(), [self loadRates]];
  [state writeToURL:self.stateURL atomically:YES encoding:NSUTF8StringEncoding error:NULL];
  return YES;
}

@end
"#;

const MAIN_M: &str = r#"#import <UIKit/UIKit.h>

#import "till/TillAppDelegate.h"

int main(int argc, char *argv[]) {
  @autoreleasepool {
    return UIApplicationMain(argc, argv, nil, NSStringFromClass([TillAppDelegate class]));
  }
}
"#;

const TESTS_M: &str = r#"#import <XCTest/XCTest.h>

#import <Share/share_api.h>

#import "till/TillAppDelegate.h"

@interface TillAppTests : XCTestCase
@end

@implementation TillAppTests

- (void)testFrameworkCallTotalsLines {
  XCTAssertEqual(share_total("tea 450\nbread 325\n"), 775);
  XCTAssertEqual(share_total("broken"), -1);
}

- (void)testFrameworkRunsOnTheSimulator {
  XCTAssertEqualObjects([NSString stringWithUTF8String:share_platform()], @"iphonesimulator");
}

- (void)testFrameworkBundleCarriesItsResource {
  NSBundle *share = [NSBundle bundleWithIdentifier:@"com.example.share"];
  XCTAssertNotNil(share);
  XCTAssertTrue([share.bundlePath hasSuffix:@"Frameworks/Share.framework"]);
  XCTAssertNotNil([share pathForResource:@"rates" ofType:@"txt"]);
}

- (void)testHostAppTotalsTheFrameworkRates {
  TillAppDelegate *till = [[TillAppDelegate alloc] init];
  XCTAssertEqual([till loadRates], 1375);
}

@end
"#;

const BROKEN_BUILD: &str = r#"load("@apple_rules//apple:apple.bzl", "apple_dynamic_xcframework_import")
load("@dx//apple/ios:app.bzl", "apple_ios_app")

apple_dynamic_xcframework_import(
    name = "share",
    xcframework_imports = glob(["Share.xcframework/**"]),
)

apple_ios_app(
    name = "app",
    srcs = ["main.m"],
    bundle_id = "com.example.broken",
    deployment = "17.0",
    environment = "simulator",
    frameworks = [":share"],
    version = "1.0",
)
"#;

const BROKEN_MAIN_M: &str = r#"#import <UIKit/UIKit.h>

#import <Share/share_api.h>

int main(int argc, char *argv[]) {
  @autoreleasepool {
    return share_total("tea 450\n") == 450 ? UIApplicationMain(argc, argv, nil, nil) : 1;
  }
}
"#;

fn keep_file(name: &str, from: &Path) {
    if let Some(dir) = outputs() {
        std::fs::create_dir_all(&dir).unwrap_or_else(|error| panic!("mkdir: {error}"));
        std::fs::copy(from, dir.join(name)).unwrap_or_else(|error| panic!("{from:?}: {error}"));
    }
}

fn producer(xcode: &str) -> Case {
    let root = consumer("apple-framework-producer", PRODUCER_EXTRA, "");
    write(&root.join("BUILD.bazel"), "");
    write(
        &root.join("share/BUILD.bazel"),
        &SHARE_BUILD
            .replace("SLICES", SLICES)
            .replace("XCODE", xcode),
    );
    write(&root.join("share/share.rs"), SHARE_RS);
    write(&root.join("share/share_api.h"), SHARE_H);
    write(&root.join("share/rates.txt"), RATES);
    root
}

fn unzip(archive: &Path, into: &Path) {
    std::fs::create_dir_all(into).unwrap_or_else(|error| panic!("mkdir {into:?}: {error}"));
    tool(
        "unzip",
        &[
            "-q",
            &archive.to_string_lossy(),
            "-d",
            &into.to_string_lossy(),
        ],
    );
}

fn plist(file: &Path, key: &str) -> String {
    tool(
        "plutil",
        &["-extract", key, "raw", "-o", "-", &file.to_string_lossy()],
    )
}

fn loads(binary: &Path, arch: &str) -> String {
    tool(
        "xcrun",
        &["otool", "-arch", arch, "-l", &binary.to_string_lossy()],
    )
}

fn field(loads: &str, name: &str) -> Vec<String> {
    let mut found: Vec<String> = loads
        .lines()
        .map(str::split_whitespace)
        .filter_map(|mut words| match (words.next(), words.next()) {
            (Some(key), Some(value)) if key == name => Some(value.to_owned()),
            _ => None,
        })
        .collect();
    found.sort();
    found.dedup();
    found
}

fn libraries(root: &Path) -> Vec<(String, String, Vec<String>)> {
    let info = root.join("Info.plist");
    let mut found = Vec::new();
    for index in 0.. {
        let entry = format!("AvailableLibraries.{index}");
        let probe = Command::new("plutil")
            .args([
                "-extract",
                &format!("{entry}.LibraryIdentifier"),
                "raw",
                "-o",
                "-",
                &info.to_string_lossy(),
            ])
            .output()
            .unwrap_or_else(|error| panic!("plutil: {error}"));
        if !probe.status.success() {
            break;
        }
        let identifier = String::from_utf8_lossy(&probe.stdout).trim().to_owned();
        let variant = Command::new("plutil")
            .args([
                "-extract",
                &format!("{entry}.SupportedPlatformVariant"),
                "raw",
                "-o",
                "-",
                &info.to_string_lossy(),
            ])
            .output()
            .ok()
            .filter(|output| output.status.success())
            .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
            .unwrap_or_default();
        let mut archs = Vec::new();
        for position in 0.. {
            let arch = Command::new("plutil")
                .args([
                    "-extract",
                    &format!("{entry}.SupportedArchitectures.{position}"),
                    "raw",
                    "-o",
                    "-",
                    &info.to_string_lossy(),
                ])
                .output()
                .unwrap_or_else(|error| panic!("plutil: {error}"));
            if !arch.status.success() {
                break;
            }
            archs.push(String::from_utf8_lossy(&arch.stdout).trim().to_owned());
        }
        archs.sort();
        found.push((identifier, variant, archs));
    }
    found.sort();
    found
}

fn inspect_slice(framework: &Path, arch: &str, platform: &str, deployment: &str) {
    let binary = framework.join("Share");
    let loads = loads(&binary, arch);
    keep(
        &format!(
            "{}-{arch}.otool.txt",
            framework
                .parent()
                .and_then(Path::file_name)
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default()
        ),
        &loads,
    );
    assert_eq!(field(&loads, "platform"), [platform], "{loads}");
    assert_eq!(field(&loads, "minos"), [deployment], "{loads}");
    assert!(
        loads.contains("name @rpath/Share.framework/Share (offset"),
        "{loads}"
    );
    let symbols = tool(
        "xcrun",
        &["nm", "-gU", "-arch", arch, &binary.to_string_lossy()],
    );
    assert!(symbols.contains(" _share_total"), "{symbols}");
    assert!(symbols.contains(" _share_platform"), "{symbols}");
}

fn inspect_framework(framework: &Path, supported: &str) {
    let info = framework.join("Info.plist");
    assert_eq!(plist(&info, "CFBundleIdentifier"), "com.example.share");
    assert_eq!(plist(&info, "CFBundleExecutable"), "Share");
    assert_eq!(plist(&info, "CFBundlePackageType"), "FMWK");
    assert_eq!(plist(&info, "CFBundleShortVersionString"), "1.4");
    assert_eq!(plist(&info, "MinimumOSVersion"), "17.0");
    assert_eq!(plist(&info, "CFBundleSupportedPlatforms.0"), supported);
    assert_eq!(read(&framework.join("rates.txt")), RATES);
    assert_eq!(read(&framework.join("Headers/share_api.h")), SHARE_H);
    let modulemap = read(&framework.join("Modules/module.modulemap"));
    assert!(
        modulemap.contains("framework module Share {"),
        "{modulemap}"
    );
}

fn archive(dir: &Path, name: &str, target: &str, sdk: &str) {
    let source = dir.join("marker.c");
    write(&source, MARKER_C);
    let object = dir.join(format!("{name}.o"));
    tool(
        "xcrun",
        &[
            "--sdk",
            sdk,
            "clang",
            "-target",
            target,
            "-c",
            &source.to_string_lossy(),
            "-o",
            &object.to_string_lossy(),
        ],
    );
    tool(
        "xcrun",
        &[
            "libtool",
            "-static",
            "-o",
            &dir.join(format!("lib{name}.a")).to_string_lossy(),
            &object.to_string_lossy(),
        ],
    );
    std::fs::remove_file(&object).unwrap_or_else(|error| panic!("{object:?}: {error}"));
}

fn write_bytes(to: &Path, from: &Path) {
    let bytes = std::fs::read(from).unwrap_or_else(|error| panic!("{from:?}: {error}"));
    std::fs::write(to, bytes).unwrap_or_else(|error| panic!("{to:?}: {error}"));
}

fn check_tampered(archive: &Path, case: &str, drop: Option<&str>, xcode: &str) -> Run {
    let dir = mkscratch(&format!("apple-xcframework-{case}"))
        .unwrap_or_else(|error| panic!("scratch: {error}"));
    let copy = dir.join("Share_bundle.xcframework.zip");
    write_bytes(&copy, archive);
    if let Some(entry) = drop {
        tool("zip", &["-q", "-d", &copy.to_string_lossy(), entry]);
    }
    let report = dir.join("Share_xcframework.txt");
    let out = dir.join("Share.xcframework.zip");
    let script = resolve_runfiles(
        &std::env::var("DX_XCFRAMEWORK_CHECK")
            .unwrap_or_else(|_| panic!("DX_XCFRAMEWORK_CHECK must name a script")),
    );
    let ran = run(
        &script,
        &[
            &report.to_string_lossy(),
            &out.to_string_lossy(),
            &copy.to_string_lossy(),
            "Share",
            "com.example.share",
            "1.4",
            "18.0",
            xcode,
            "device/arm64,simulator/arm64",
            "rates.txt",
            "share_api.h",
        ],
        &[],
    )
    .unwrap_or_else(|error| panic!("run: {error}"));
    assert_eq!(
        out.exists(),
        ran.status.success(),
        "{case}: only a clean XCFramework is published"
    );
    ran
}

fn broken_app(root: &Path, package: &str, xcframework: &Path) {
    let dir = root.join(package);
    write(&dir.join("BUILD.bazel"), BROKEN_BUILD);
    write(&dir.join("main.m"), BROKEN_MAIN_M);
    tool(
        "cp",
        &[
            "-R",
            &xcframework.to_string_lossy(),
            &dir.join("Share.xcframework").to_string_lossy(),
        ],
    );
}

#[test]
fn a_rust_xcframework_is_built_inspected_and_consumed_by_an_independent_app() {
    let xcode = installed_xcode();
    let producer = producer(&xcode);

    passes(&bazel(&producer, "build", &["//share:share"], &[]));
    let report = read(&producer.join("bazel-bin/share/share_xcframework.txt"));
    keep("share_xcframework.txt", &report);
    assert!(report.contains("xcframework=Share\n"), "{report}");
    assert!(report.contains("resources=rates.txt\n"), "{report}");
    for (cell, library, platform) in [
        ("device/arm64", "ios-arm64", "iphoneos"),
        (
            "simulator/arm64",
            "ios-arm64_x86_64-simulator",
            "iphonesimulator",
        ),
        (
            "simulator/x86_64",
            "ios-arm64_x86_64-simulator",
            "iphonesimulator",
        ),
    ] {
        assert!(
            report.contains(&format!(
                "slice={cell} library={library} platform={platform} minos=17.0 install_name=@rpath/Share.framework/Share\n"
            )),
            "{report}"
        );
    }
    let zip = producer.join("bazel-bin/share/share.xcframework.zip");
    keep_file("Share.xcframework.zip", &zip);

    let unpacked =
        mkscratch("apple-xcframework").unwrap_or_else(|error| panic!("scratch: {error}"));
    unzip(&zip, &unpacked);
    let xcframework = unpacked.join("Share.xcframework");
    assert_eq!(
        libraries(&xcframework),
        [
            (
                "ios-arm64".to_owned(),
                String::new(),
                vec!["arm64".to_owned()]
            ),
            (
                "ios-arm64_x86_64-simulator".to_owned(),
                "simulator".to_owned(),
                vec!["arm64".to_owned(), "x86_64".to_owned()]
            ),
        ]
    );
    let device = xcframework.join("ios-arm64/Share.framework");
    let simulator = xcframework.join("ios-arm64_x86_64-simulator/Share.framework");
    inspect_framework(&device, "iPhoneOS");
    inspect_framework(&simulator, "iPhoneSimulator");
    assert_eq!(
        tool(
            "xcrun",
            &["lipo", "-archs", &device.join("Share").to_string_lossy()]
        ),
        "arm64"
    );
    let mut simulator_archs: Vec<String> = tool(
        "xcrun",
        &["lipo", "-archs", &simulator.join("Share").to_string_lossy()],
    )
    .split_whitespace()
    .map(str::to_owned)
    .collect();
    simulator_archs.sort();
    assert_eq!(simulator_archs, ["arm64", "x86_64"]);
    inspect_slice(&device, "arm64", "2", "17.0");
    inspect_slice(&simulator, "arm64", "7", "17.0");
    inspect_slice(&simulator, "x86_64", "7", "17.0");

    let sdk_version = tool("xcrun", &["--sdk", "iphonesimulator", "--show-sdk-version"]);
    let runtime = runtime_for(&sdk_version);
    let simulator_device = Simulator::create("dx-till", &runtime);
    let till = consumer("apple-framework-consumer", CONSUMER_EXTRA, CONSUMER_BAZELRC);
    write(&till.join("BUILD.bazel"), "");
    unzip(&zip, &till.join("till"));
    write(
        &till.join("till/BUILD.bazel"),
        &TILL_BUILD
            .replace("XCODE", &xcode)
            .replace("RUNTIME", &runtime)
            .replace("SIMULATOR", &simulator_device.name),
    );
    write(&till.join("till/TillAppDelegate.h"), DELEGATE_H);
    write(&till.join("till/TillAppDelegate.m"), DELEGATE_M);
    write(&till.join("till/main.m"), MAIN_M);
    write(&till.join("till/TillAppTests.m"), TESTS_M);
    assert!(
        !till.join("till/share.rs").exists()
            && !read(&till.join("MODULE.bazel")).contains("rules_rust"),
        "the consumer must not see the producer sources"
    );

    passes(&bazel(&till, "build", &["//till:till_app"], &[]));
    let cell = read(&till.join("bazel-bin/till/till_app_library.txt"));
    keep("till_app_library.txt", &cell);
    assert!(cell.contains("cell=simulator/arm64\n"), "{cell}");
    assert!(
        cell.contains("framework=Share arch=arm64 platform=iphonesimulator minos=17.0\n"),
        "{cell}"
    );

    let ipa = mkscratch("apple-till-ipa").unwrap_or_else(|error| panic!("scratch: {error}"));
    unzip(&till.join("bazel-bin/till/till_app.ipa"), &ipa);
    let app = ipa.join("Payload/till_app.app");
    let embedded = app.join("Frameworks/Share.framework");
    assert_eq!(read(&embedded.join("rates.txt")), RATES);
    assert_eq!(
        plist(&embedded.join("Info.plist"), "CFBundleIdentifier"),
        "com.example.share"
    );
    assert_eq!(
        tool(
            "xcrun",
            &["lipo", "-archs", &embedded.join("Share").to_string_lossy()]
        ),
        "arm64"
    );
    let binary = app.join("till_app").to_string_lossy().into_owned();
    let linked = tool("xcrun", &["otool", "-L", &binary]);
    keep("till_app.otool-L.txt", &linked);
    assert!(linked.contains("@rpath/Share.framework/Share"), "{linked}");
    let app_loads = tool("xcrun", &["otool", "-l", &binary]);
    assert!(
        app_loads.contains("path @executable_path/Frameworks"),
        "{app_loads}"
    );
    let undefined = tool("xcrun", &["nm", "-u", &binary]);
    assert!(undefined.contains("_share_total"), "{undefined}");
    let defined = tool("xcrun", &["nm", "-gU", &binary]);
    assert!(!defined.contains(" _share_total"), "{defined}");

    let tested = bazel(
        &till,
        "test",
        &["//till:till_app_test"],
        &["--test_output=all"],
    );
    passes(&tested);
    for case in [
        "testFrameworkCallTotalsLines",
        "testFrameworkRunsOnTheSimulator",
        "testFrameworkBundleCarriesItsResource",
        "testHostAppTotalsTheFrameworkRates",
    ] {
        assert!(
            passed(&tested, case),
            "{case} did not pass:\n{}",
            tested.combined()
        );
    }

    passes(&bazel(&till, "run", &["//till:boot"], &[]));
    passes(&bazel(&till, "run", &["//till:install"], &[]));
    let container = tool(
        "xcrun",
        &[
            "simctl",
            "get_app_container",
            &simulator_device.udid,
            "com.example.till",
        ],
    );
    assert!(
        Path::new(&container)
            .join("Frameworks/Share.framework/rates.txt")
            .is_file(),
        "{container}"
    );
    passes(&bazel(&till, "run", &["//till:launch"], &[]));
    let data = tool(
        "xcrun",
        &[
            "simctl",
            "get_app_container",
            &simulator_device.udid,
            "com.example.till",
            "data",
        ],
    );
    let state = wait_for(&Path::new(&data).join("Documents/till-state.txt"));
    assert_eq!(state, "platform=iphonesimulator\ntotal=1375\n");
    keep(
        "framework-facts.txt",
        &format!(
            "xcode={xcode}\nsdk_version={sdk_version}\nruntime=iOS {runtime}\nsimulator={}\nhost_arch={}\nstate={}\n",
            simulator_device.udid,
            tool("uname", &["-m"]),
            state.replace('\n', ";")
        ),
    );
}

#[test]
fn malformed_and_incompatible_frameworks_fail_explicitly() {
    let xcode = installed_xcode();
    let producer = producer(&xcode);
    let bad = producer.join("bad");
    archive(&bad, "device", "arm64-apple-ios17.0", "iphoneos");
    archive(
        &bad,
        "simulator",
        "arm64-apple-ios17.0-simulator",
        "iphonesimulator",
    );
    archive(&bad, "device18", "arm64-apple-ios18.0", "iphoneos");
    archive(
        &bad,
        "simulator18",
        "arm64-apple-ios18.0-simulator",
        "iphonesimulator",
    );
    write(&bad.join("BUILD.bazel"), BAD_BUILD);
    write(&bad.join("marker.h"), MARKER_H);

    let device = bazel(&producer, "build", &["//bad:device_slice"], &[]);
    fails_with(
        &device,
        "apple_ios_framework: library archive libdevice.a is built for iphoneos, want iphonesimulator for simulator/arm64",
    );
    assert!(!producer
        .join("bazel-bin/bad/device_slice.xcframework.zip")
        .exists());

    let newer = bazel(&producer, "build", &["//bad:newer_archive"], &[]);
    fails_with(
        &newer,
        "needs iOS 18.0, above the framework deployment 17.0",
    );
    assert!(!producer
        .join("bazel-bin/bad/newer_archive.xcframework.zip")
        .exists());

    let duplicate = bazel(&producer, "build", &["//bad:duplicate_slices"], &[]);
    fails_with(
        &duplicate,
        "apple_framework: duplicate slice 'simulator/arm64'",
    );

    passes(&bazel(
        &producer,
        "build",
        &["//share:newer", "//share:partial"],
        &[],
    ));
    let newer_report = read(&producer.join("bazel-bin/share/newer_xcframework.txt"));
    keep("newer_xcframework.txt", &newer_report);
    assert!(
        newer_report.contains(
            "slice=simulator/arm64 library=ios-arm64-simulator platform=iphonesimulator minos=18.0"
        ),
        "{newer_report}"
    );
    let partial_report = read(&producer.join("bazel-bin/share/partial_xcframework.txt"));
    keep("partial_xcframework.txt", &partial_report);
    assert!(
        partial_report.contains("slice=simulator/x86_64 library=ios-x86_64-simulator platform=iphonesimulator minos=17.0"),
        "{partial_report}"
    );

    let newer_zip = producer.join("bazel-bin/share/newer.xcframework.zip");
    let xcode_line = newer_report
        .lines()
        .find_map(|line| line.strip_prefix("xcode="))
        .unwrap_or_else(|| panic!("no xcode line in {newer_report}"))
        .to_owned();
    let untouched = check_tampered(&newer_zip, "untouched", None, &xcode_line);
    assert_eq!(untouched.status.code(), Some(0), "{}", untouched.combined());
    for (case, entry, want) in [
        (
            "resource",
            "Share.xcframework/ios-arm64-simulator/Share.framework/rates.txt",
            "apple_ios_framework: library ios-arm64-simulator misses resource Share.framework/rates.txt\n",
        ),
        (
            "modulemap",
            "Share.xcframework/ios-arm64/Share.framework/Modules/module.modulemap",
            "apple_ios_framework: library ios-arm64 has no Share.framework/Modules/module.modulemap\n",
        ),
    ] {
        let ran = check_tampered(&newer_zip, case, Some(entry), &xcode_line);
        keep(&format!("check-{case}.txt"), &ran.combined());
        assert_eq!(ran.status.code(), Some(1), "{}", ran.combined());
        assert_eq!(ran.stderr, want);
    }

    let unpacked =
        mkscratch("apple-xcframework-bad").unwrap_or_else(|error| panic!("scratch: {error}"));
    unzip(
        &producer.join("bazel-bin/share/newer.xcframework.zip"),
        &unpacked.join("newer"),
    );
    unzip(
        &producer.join("bazel-bin/share/partial.xcframework.zip"),
        &unpacked.join("partial"),
    );
    unzip(
        &producer.join("bazel-bin/share/newer.xcframework.zip"),
        &unpacked.join("swapped"),
    );
    let swapped = unpacked.join("swapped/Share.xcframework");
    std::fs::copy(
        swapped.join("ios-arm64/Share.framework/Share"),
        swapped.join("ios-arm64-simulator/Share.framework/Share"),
    )
    .unwrap_or_else(|error| panic!("swap slices: {error}"));

    let till = consumer(
        "apple-framework-bad-consumer",
        CONSUMER_EXTRA,
        CONSUMER_BAZELRC,
    );
    write(&till.join("BUILD.bazel"), "");
    broken_app(&till, "newer", &unpacked.join("newer/Share.xcframework"));
    broken_app(
        &till,
        "partial",
        &unpacked.join("partial/Share.xcframework"),
    );
    broken_app(&till, "swapped", &swapped);

    let newer = bazel(&till, "build", &["//newer:app"], &[]);
    fails_with(
        &newer,
        "apple_ios_app: framework binary Share needs iOS 18.0, above the app deployment 17.0",
    );
    assert!(!till.join("bazel-bin/newer/app.ipa").exists());

    let partial = bazel(&till, "build", &["//partial:app"], &[]);
    fails_with(
        &partial,
        "Imported XCFramework does not support the following platform",
    );
    assert!(!till.join("bazel-bin/partial/app.ipa").exists());

    let swapped = bazel(&till, "build", &["//swapped:app"], &[]);
    fails_with(
        &swapped,
        "apple_ios_app: framework binary Share is built for iphoneos, want iphonesimulator for simulator/arm64",
    );
    assert!(!till.join("bazel-bin/swapped/app.ipa").exists());
}
