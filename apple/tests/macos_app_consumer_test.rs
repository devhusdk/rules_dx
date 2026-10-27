mod consumer;
mod macos;

use std::path::{Path, PathBuf};

use consumer::{consumer, fails_with, read, write};
use dx_nested::Case;
use dx_testing::mkscratch;
use macos::{
    bazel, installed_xcode, keep, passed, passes, runtime_for, tool, version, wait_for, Simulator,
};

const MODULE_EXTRA: &str = r#"
bazel_dep(name = "rules_rust", version = "0.74.0")

apple_cc_configure = use_extension("@xcode_support//crosstool:setup.bzl", "apple_cc_configure_extension")
use_repo(apple_cc_configure, "local_config_apple_cc_toolchains")

register_toolchains("@local_config_apple_cc_toolchains//:all")

rust = use_extension("@rules_rust//rust:extensions.bzl", "rust")
rust.toolchain(
    edition = "2021",
    extra_target_triples = ["aarch64-apple-ios-sim"],
    versions = ["1.98.0"],
)
use_repo(rust, "rust_toolchains")

register_toolchains("@rust_toolchains//:all")
"#;

const BAZELRC: &str = "build --ios_multi_cpus=sim_arm64\n";

const BUNDLE_ID: &str = "com.example.ledger";

const LEDGER_BUILD: &str = r#"load("@dx//apple/ios:app.bzl", "apple_ios_app", "apple_ios_app_test")
load("@dx//apple/ios:simulator.bzl", "apple_simulator_boot", "apple_simulator_install", "apple_simulator_launch")
load("@rules_cc//cc:objc_library.bzl", "objc_library")
load("@rules_rust//rust:defs.bzl", "rust_static_library")

rust_static_library(
    name = "ledger_rust",
    srcs = ["ledger.rs"],
    edition = "2021",
)

objc_library(
    name = "ledger_headers",
    hdrs = [
        "LedgerAppDelegate.h",
        "ledger.h",
    ],
)

apple_ios_app(
    name = "ledger_app",
    srcs = [
        "LedgerAppDelegate.m",
        "main.m",
    ],
    arch = "arm64",
    bundle_id = "com.example.ledger",
    deployment = "17.0",
    environment = "simulator",
    library = ":ledger_rust",
    resources = ["entries.txt"],
    version = "1.4",
    xcode_version = "XCODE",
    deps = [":ledger_headers"],
)

apple_ios_app_test(
    name = "ledger_app_test",
    size = "large",
    srcs = ["LedgerAppTests.m"],
    app = ":ledger_app",
    deployment = "17.0",
    device_type = "iPhone 16",
    os_version = "RUNTIME",
    deps = [":ledger_headers"],
)

apple_simulator_boot(
    name = "boot",
    simulator = "SIMULATOR",
)

apple_simulator_install(
    name = "install",
    app = ":ledger_app",
    simulator = "SIMULATOR",
)

apple_simulator_launch(
    name = "launch",
    bundle_id = "com.example.ledger",
    simulator = "SIMULATOR",
)

apple_simulator_boot(
    name = "boot_missing",
    simulator = "dx-no-such-simulator",
)
"#;

const LEDGER_RS: &str = r#"use std::ffi::{c_char, CStr};

#[no_mangle]
pub extern "C" fn ledger_total(text: *const c_char) -> i64 {
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
pub extern "C" fn ledger_platform() -> *const c_char {
    if cfg!(all(target_os = "ios", target_abi = "sim")) {
        c"iphonesimulator".as_ptr()
    } else {
        c"other".as_ptr()
    }
}
"#;

const LEDGER_H: &str = r#"#pragma once

#include <stdint.h>

int64_t ledger_total(const char *text);
const char *ledger_platform(void);
"#;

const DELEGATE_H: &str = r#"#import <UIKit/UIKit.h>

@interface LedgerAppDelegate : UIResponder <UIApplicationDelegate>
@property(nonatomic, strong) UIWindow *window;
@property(nonatomic, readonly) int64_t total;
@property(nonatomic, readonly, copy) NSString *entries;
- (BOOL)loadEntries;
- (int64_t)addEntry:(NSString *)name cents:(int64_t)cents;
- (NSString *)platform;
- (NSURL *)stateURL;
@end
"#;

const DELEGATE_M: &str = r#"#import "ledger/LedgerAppDelegate.h"

#import "ledger/ledger.h"

@implementation LedgerAppDelegate {
  UILabel *_label;
}

- (BOOL)loadEntries {
  NSString *path = [[NSBundle mainBundle] pathForResource:@"entries" ofType:@"txt"];
  if (path == nil) {
    return NO;
  }
  NSString *text = [NSString stringWithContentsOfFile:path encoding:NSUTF8StringEncoding error:NULL];
  if (text == nil) {
    return NO;
  }
  _entries = [text copy];
  _total = ledger_total(_entries.UTF8String);
  return _total >= 0;
}

- (int64_t)addEntry:(NSString *)name cents:(int64_t)cents {
  _entries = [_entries stringByAppendingFormat:@"%@ %lld\n", name, cents];
  _total = ledger_total(_entries.UTF8String);
  [self persist];
  return _total;
}

- (NSString *)platform {
  return [NSString stringWithUTF8String:ledger_platform()];
}

- (NSURL *)stateURL {
  NSURL *documents = [[NSFileManager defaultManager] URLsForDirectory:NSDocumentDirectory
                                                            inDomains:NSUserDomainMask]
                         .firstObject;
  return [documents URLByAppendingPathComponent:@"ledger-state.txt"];
}

- (void)persist {
  NSString *state = [NSString stringWithFormat:@"platform=%@\ntotal=%lld\n", self.platform, _total];
  [state writeToURL:self.stateURL atomically:YES encoding:NSUTF8StringEncoding error:NULL];
  _label.text = [NSString stringWithFormat:@"%lld cents", _total];
}

- (BOOL)application:(UIApplication *)application
    didFinishLaunchingWithOptions:(NSDictionary *)launchOptions {
  BOOL loaded = [self loadEntries];
  UIViewController *root = [[UIViewController alloc] init];
  _label = [[UILabel alloc] initWithFrame:CGRectMake(20, 80, 280, 40)];
  [root.view addSubview:_label];
  self.window = [[UIWindow alloc] initWithFrame:[[UIScreen mainScreen] bounds]];
  self.window.rootViewController = root;
  [self.window makeKeyAndVisible];
  if (loaded) {
    [self persist];
  }
  return YES;
}

@end
"#;

const MAIN_M: &str = r#"#import <UIKit/UIKit.h>

#import "ledger/LedgerAppDelegate.h"

int main(int argc, char *argv[]) {
  @autoreleasepool {
    return UIApplicationMain(argc, argv, nil, NSStringFromClass([LedgerAppDelegate class]));
  }
}
"#;

const ENTRIES: &str = "tea 450\nbread 325\njam 600\n";

const TESTS_M: &str = r#"#import <XCTest/XCTest.h>

#import "ledger/LedgerAppDelegate.h"
#import "ledger/ledger.h"

@interface LedgerAppTests : XCTestCase
@end

@implementation LedgerAppTests

- (void)testRustTotalsLedgerLines {
  XCTAssertEqual(ledger_total("tea 450\nbread 325\n"), 775);
  XCTAssertEqual(ledger_total("broken"), -1);
}

- (void)testRustRunsOnTheSimulator {
  XCTAssertEqualObjects([NSString stringWithUTF8String:ledger_platform()], @"iphonesimulator");
}

- (void)testHostAppLoadedTheBundledEntries {
  id delegate = UIApplication.sharedApplication.delegate;
  XCTAssertTrue([delegate isKindOfClass:[LedgerAppDelegate class]]);
  NSString *path = [[NSBundle mainBundle] pathForResource:@"entries" ofType:@"txt"];
  XCTAssertNotNil(path);
  LedgerAppDelegate *ledger = [[LedgerAppDelegate alloc] init];
  XCTAssertTrue([ledger loadEntries]);
  XCTAssertEqual(ledger.total, 1375);
}

- (void)testAddingAnEntryChangesTheLedgerState {
  LedgerAppDelegate *ledger = [[LedgerAppDelegate alloc] init];
  XCTAssertTrue([ledger loadEntries]);
  XCTAssertEqual([ledger addEntry:@"cake" cents:500], 1875);
  XCTAssertEqual(ledger.total, 1875);
  NSString *state = [NSString stringWithContentsOfURL:ledger.stateURL
                                             encoding:NSUTF8StringEncoding
                                                error:NULL];
  XCTAssertEqualObjects(state, @"platform=iphonesimulator\ntotal=1875\n");
}

@end
"#;

const BAD_BUILD: &str = r#"load("@dx//apple/ios:app.bzl", "apple_ios_app", "apple_ios_app_test")
load("@rules_cc//cc:defs.bzl", "cc_import")

cc_import(
    name = "device_lib",
    static_library = "libdevice.a",
)

cc_import(
    name = "simulator_lib",
    static_library = "libsimulator.a",
)

apple_ios_app(
    name = "device_slice_app",
    srcs = ["main.m"],
    bundle_id = "com.example.device",
    deployment = "17.0",
    environment = "simulator",
    library = ":device_lib",
    version = "1.0",
)

apple_ios_app(
    name = "simulator_app",
    srcs = ["main.m"],
    bundle_id = "com.example.simulator",
    deployment = "17.0",
    environment = "simulator",
    library = ":simulator_lib",
    version = "1.0",
)

apple_ios_app(
    name = "missing_resource_app",
    srcs = ["main.m"],
    bundle_id = "com.example.missing",
    deployment = "17.0",
    environment = "simulator",
    library = ":simulator_lib",
    resources = ["missing.txt"],
    version = "1.0",
)

apple_ios_app(
    name = "device_app",
    srcs = ["main.m"],
    bundle_id = "com.example.device",
    deployment = "17.0",
    environment = "device",
    library = ":device_lib",
    provisioning_profile = "dev.mobileprovision",
    version = "1.0",
)

apple_ios_app(
    name = "expired_device_app",
    srcs = ["main.m"],
    bundle_id = "com.example.device",
    deployment = "17.0",
    environment = "device",
    library = ":device_lib",
    provisioning_profile = "expired.mobileprovision",
    version = "1.0",
)

apple_ios_app_test(
    name = "missing_runtime_test",
    srcs = ["empty_test.m"],
    app = ":simulator_app",
    deployment = "17.0",
    device_type = "iPhone 16",
    os_version = "17.9",
)
"#;

const BAD_MAIN_M: &str = r#"#import <UIKit/UIKit.h>

int ledger_marker(void);

int main(int argc, char *argv[]) {
  @autoreleasepool {
    return ledger_marker() == 7 ? UIApplicationMain(argc, argv, nil, nil) : 1;
  }
}
"#;

const BAD_TEST_M: &str = r#"#import <XCTest/XCTest.h>

@interface EmptyTests : XCTestCase
@end

@implementation EmptyTests
- (void)testNothing {
  XCTAssertTrue(YES);
}
@end
"#;

const MARKER_C: &str = "int ledger_marker(void) { return 7; }\n";

fn ledger(xcode: &str, runtime: &str, simulator: &str) -> Case {
    let root = consumer("apple-app-consumer", MODULE_EXTRA, BAZELRC);
    write(&root.join("BUILD.bazel"), "");
    write(
        &root.join("ledger/BUILD.bazel"),
        &LEDGER_BUILD
            .replace("XCODE", xcode)
            .replace("RUNTIME", runtime)
            .replace("SIMULATOR", simulator),
    );
    write(&root.join("ledger/ledger.rs"), LEDGER_RS);
    write(&root.join("ledger/ledger.h"), LEDGER_H);
    write(&root.join("ledger/LedgerAppDelegate.h"), DELEGATE_H);
    write(&root.join("ledger/LedgerAppDelegate.m"), DELEGATE_M);
    write(&root.join("ledger/main.m"), MAIN_M);
    write(&root.join("ledger/entries.txt"), ENTRIES);
    write(&root.join("ledger/LedgerAppTests.m"), TESTS_M);
    root
}

fn archive(root: &Path, name: &str, target: &str, sdk: &str) {
    let dir = root.join("bad");
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

fn unpack(ipa: &Path) -> PathBuf {
    let dir = mkscratch("apple-app-ipa").unwrap_or_else(|error| panic!("scratch: {error}"));
    tool(
        "unzip",
        &["-q", &ipa.to_string_lossy(), "-d", &dir.to_string_lossy()],
    );
    dir.join("Payload/ledger_app.app")
}

/// Names of simulators created by `apple_ios_app_test` (`dx ... pgid N`).
fn dx_owned_simulators() -> Vec<String> {
    tool("xcrun", &["simctl", "list", "devices"])
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("dx ") && line.contains(" pgid "))
        .map(str::to_string)
        .collect()
}

fn plist(app: &Path, key: &str) -> String {
    tool(
        "plutil",
        &[
            "-extract",
            key,
            "raw",
            "-o",
            "-",
            &app.join("Info.plist").to_string_lossy(),
        ],
    )
}

#[test]
fn a_rust_app_builds_tests_installs_and_launches_on_a_simulator() {
    let xcode = installed_xcode();
    let sdk_version = tool("xcrun", &["--sdk", "iphonesimulator", "--show-sdk-version"]);
    let runtime = runtime_for(&sdk_version);
    let simulator = Simulator::create("dx-ledger", &runtime);
    let root = ledger(&xcode, &runtime, &simulator.name);

    let booted_before = tool("xcrun", &["simctl", "list", "devices", "booted"]);
    let built = bazel(&root, "build", &["//ledger:ledger_app"], &[]);
    passes(&built);
    let booted = tool("xcrun", &["simctl", "list", "devices", "booted"]);
    assert_eq!(
        booted, booted_before,
        "building changed the booted simulators"
    );
    let listed = tool("xcrun", &["simctl", "list", "devices"]);
    assert!(
        listed
            .lines()
            .any(|line| line.contains(&simulator.udid) && line.contains("(Shutdown)")),
        "{} should exist and be shut down:\n{listed}",
        simulator.udid
    );

    let cell = read(&root.join("bazel-bin/ledger/ledger_app_library.txt"));
    keep("ledger_app_library.txt", &cell);
    assert!(cell.contains("cell=simulator/arm64\n"), "{cell}");
    assert!(cell.contains("sdk=iphonesimulator\n"), "{cell}");
    assert!(
        cell.contains(&format!("sdk_version={sdk_version}\n")),
        "{cell}"
    );
    assert!(cell.contains("deployment=17.0\n"), "{cell}");
    assert!(cell.contains(&format!("xcode={xcode}")), "{cell}");
    let rust_minos = cell
        .lines()
        .filter(|line| line.starts_with("archive=") && line.contains("ledger_rust"))
        .find_map(|line| line.split_once(" arch=arm64 platform=iphonesimulator minos="))
        .map(|(_, minos)| version(minos))
        .unwrap_or_else(|| panic!("no simulator Rust archive line:\n{cell}"));
    assert!(rust_minos <= version("17.0"), "{cell}");

    let app = unpack(&root.join("bazel-bin/ledger/ledger_app.ipa"));
    assert_eq!(plist(&app, "CFBundleIdentifier"), BUNDLE_ID);
    assert_eq!(plist(&app, "CFBundleExecutable"), "ledger_app");
    assert_eq!(plist(&app, "CFBundleShortVersionString"), "1.4");
    assert_eq!(plist(&app, "MinimumOSVersion"), "17.0");
    assert_eq!(plist(&app, "DTPlatformName"), "iphonesimulator");
    assert_eq!(
        plist(&app, "CFBundleSupportedPlatforms.0"),
        "iPhoneSimulator"
    );
    assert_eq!(read(&app.join("entries.txt")), ENTRIES);
    let binary = app.join("ledger_app");
    let binary_path = binary.to_string_lossy().into_owned();
    assert_eq!(tool("xcrun", &["lipo", "-archs", &binary_path]), "arm64");
    let loads = tool("xcrun", &["otool", "-l", &binary_path]);
    keep("ledger_app.otool.txt", &loads);
    assert!(loads.contains("platform 7"), "{loads}");
    assert!(loads.contains("minos 17.0"), "{loads}");
    assert!(loads.contains(&format!("sdk {sdk_version}")), "{loads}");
    let symbols = tool("xcrun", &["nm", "-gU", &binary_path]);
    assert!(symbols.contains(" _ledger_total"), "{symbols}");
    assert!(symbols.contains(" _ledger_platform"), "{symbols}");

    let owned_before = dx_owned_simulators();
    let tested = bazel(
        &root,
        "test",
        &["//ledger:ledger_app_test"],
        &["--test_output=all"],
    );
    passes(&tested);
    for case in [
        "testRustTotalsLedgerLines",
        "testRustRunsOnTheSimulator",
        "testHostAppLoadedTheBundledEntries",
        "testAddingAnEntryChangesTheLedgerState",
    ] {
        assert!(
            passed(&tested, case),
            "{case} did not pass:\n{}",
            tested.combined()
        );
    }

    let owned_after = dx_owned_simulators();
    let leaked: Vec<&String> = owned_after
        .iter()
        .filter(|name| !owned_before.contains(name))
        .collect();
    assert!(
        leaked.is_empty(),
        "the test simulator outlived its run: {leaked:?}"
    );

    passes(&bazel(&root, "run", &["//ledger:boot"], &[]));
    let booted = tool("xcrun", &["simctl", "list", "devices", "booted"]);
    assert!(booted.contains(&simulator.udid), "{booted}");
    passes(&bazel(&root, "run", &["//ledger:install"], &[]));
    let container = tool(
        "xcrun",
        &["simctl", "get_app_container", &simulator.udid, BUNDLE_ID],
    );
    assert!(
        Path::new(&container).join("entries.txt").is_file(),
        "{container}"
    );
    passes(&bazel(&root, "run", &["//ledger:launch"], &[]));
    let data = tool(
        "xcrun",
        &[
            "simctl",
            "get_app_container",
            &simulator.udid,
            BUNDLE_ID,
            "data",
        ],
    );
    let state = wait_for(&Path::new(&data).join("Documents/ledger-state.txt"));
    assert_eq!(state, "platform=iphonesimulator\ntotal=1375\n");

    let arch = tool("uname", &["-m"]);
    let facts = format!(
        "xcode={xcode}\nsdk=iphonesimulator\nsdk_version={sdk_version}\nruntime=iOS {runtime}\ndevice_type=iPhone 16\nsimulator={}\nhost_arch={arch}\napp_arch=arm64\nstate={}\n",
        simulator.udid,
        state.replace('\n', ";")
    );
    keep("simulator-facts.txt", &facts);

    let missing = bazel(&root, "run", &["//ledger:boot_missing"], &[]);
    assert_eq!(missing.status.code(), Some(2), "{}", missing.combined());
    fails_with(
        &missing,
        "dx-apple-simctl: no available simulator named 'dx-no-such-simulator'",
    );
}

const TEAM: &str = "DXTEST0001";

fn profile(dir: &Path, name: &str, expires: &str) {
    let key = dir.join("signer.key");
    let cert = dir.join("signer.pem");
    let der = dir.join("signer.der");
    if !cert.exists() {
        tool(
            "openssl",
            &[
                "req",
                "-x509",
                "-newkey",
                "rsa:2048",
                "-nodes",
                "-days",
                "2",
                "-subj",
                "/CN=dx device test",
                "-keyout",
                &key.to_string_lossy(),
                "-out",
                &cert.to_string_lossy(),
            ],
        );
        tool(
            "openssl",
            &[
                "x509",
                "-in",
                &cert.to_string_lossy(),
                "-outform",
                "DER",
                "-out",
                &der.to_string_lossy(),
            ],
        );
    }
    let encoded = tool("openssl", &["base64", "-A", "-in", &der.to_string_lossy()]);
    let plist = dir.join(format!("{name}.plist"));
    write(
        &plist,
        &format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>AppIDName</key>
  <string>dx device test</string>
  <key>ApplicationIdentifierPrefix</key>
  <array><string>{TEAM}</string></array>
  <key>CreationDate</key>
  <date>2020-01-01T00:00:00Z</date>
  <key>DeveloperCertificates</key>
  <array><data>{encoded}</data></array>
  <key>Entitlements</key>
  <dict>
    <key>application-identifier</key>
    <string>{TEAM}.com.example.device</string>
    <key>com.apple.developer.team-identifier</key>
    <string>{TEAM}</string>
    <key>get-task-allow</key>
    <true/>
  </dict>
  <key>ExpirationDate</key>
  <date>{expires}</date>
  <key>Name</key>
  <string>dx device test</string>
  <key>Platform</key>
  <array><string>iOS</string></array>
  <key>TeamIdentifier</key>
  <array><string>{TEAM}</string></array>
  <key>UUID</key>
  <string>2F6C1D7A-0B3E-4C5D-8E9F-1A2B3C4D5E6F</string>
  <key>Version</key>
  <integer>1</integer>
</dict>
</plist>
"#
        ),
    );
    tool(
        "openssl",
        &[
            "smime",
            "-sign",
            "-nodetach",
            "-binary",
            "-outform",
            "DER",
            "-in",
            &plist.to_string_lossy(),
            "-signer",
            &cert.to_string_lossy(),
            "-inkey",
            &key.to_string_lossy(),
            "-out",
            &dir.join(format!("{name}.mobileprovision"))
                .to_string_lossy(),
        ],
    );
}

#[test]
fn a_device_app_signs_ad_hoc_with_its_declared_profile() {
    let xcode = installed_xcode();
    let root = ledger(&xcode, "18.0", "unused");
    archive(&root, "device", "arm64-apple-ios17.0", "iphoneos");
    archive(
        &root,
        "simulator",
        "arm64-apple-ios17.0-simulator",
        "iphonesimulator",
    );
    write(&root.join("bad/BUILD.bazel"), BAD_BUILD);
    write(&root.join("bad/main.m"), BAD_MAIN_M);
    write(&root.join("bad/empty_test.m"), BAD_TEST_M);
    profile(&root.join("bad"), "dev", "2099-01-01T00:00:00Z");
    profile(&root.join("bad"), "expired", "2021-01-01T00:00:00Z");

    let built = bazel(
        &root,
        "build",
        &["//bad:device_app"],
        &[
            "--ios_multi_cpus=arm64",
            "--@rules_apple//apple/build_settings:signing_certificate_name=Apple Development",
        ],
    );
    passes(&built);
    let ipa = root.join("bazel-bin/bad/device_app.ipa");
    assert!(ipa.is_file(), "{}", built.combined());
    let dir = mkscratch("apple-device-ipa").unwrap_or_else(|error| panic!("scratch: {error}"));
    tool(
        "unzip",
        &["-q", &ipa.to_string_lossy(), "-d", &dir.to_string_lossy()],
    );
    let app = dir.join("Payload/device_app.app");
    assert_eq!(
        std::fs::read(app.join("embedded.mobileprovision")).ok(),
        std::fs::read(root.join("bad/dev.mobileprovision")).ok(),
        "the declared profile is embedded"
    );
    let signature = std::process::Command::new("codesign")
        .args(["-dv", &app.to_string_lossy()])
        .output()
        .unwrap_or_else(|error| panic!("codesign: {error}"));
    let details = String::from_utf8_lossy(&signature.stderr);
    assert!(details.contains("Signature=adhoc"), "{details}");

    let expired = bazel(
        &root,
        "build",
        &["//bad:expired_device_app"],
        &["--ios_multi_cpus=arm64"],
    );
    fails_with(&expired, "is in the past");
}

#[test]
fn wrong_slices_runtimes_and_resources_fail_before_success() {
    let xcode = installed_xcode();
    let root = ledger(&xcode, "18.0", "unused");
    archive(&root, "device", "arm64-apple-ios17.0", "iphoneos");
    archive(
        &root,
        "simulator",
        "arm64-apple-ios17.0-simulator",
        "iphonesimulator",
    );
    write(&root.join("bad/BUILD.bazel"), BAD_BUILD);
    write(&root.join("bad/main.m"), BAD_MAIN_M);
    write(&root.join("bad/empty_test.m"), BAD_TEST_M);

    passes(&bazel(&root, "build", &["//bad:simulator_app"], &[]));
    let cell = read(&root.join("bazel-bin/bad/simulator_app_library.txt"));
    assert!(
        cell.contains("archive=libsimulator.a arch=arm64 platform=iphonesimulator minos=17.0"),
        "{cell}"
    );

    let device = bazel(&root, "build", &["//bad:device_slice_app"], &[]);
    fails_with(
        &device,
        "apple_ios_app: library archive libdevice.a is built for iphoneos, want iphonesimulator for simulator/arm64",
    );
    assert!(
        !root.join("bazel-bin/bad/device_slice_app.ipa").exists(),
        "a rejected slice still produced an app"
    );

    let mismatch = bazel(
        &root,
        "build",
        &["//bad:simulator_app"],
        &["--ios_multi_cpus=arm64"],
    );
    fails_with(
        &mismatch,
        "apple_ios_app: app declares simulator/arm64 but this build targets device/arm64; pass --ios_multi_cpus=sim_arm64",
    );

    let resource = bazel(&root, "build", &["//bad:missing_resource_app"], &[]);
    fails_with(&resource, "missing.txt");

    let runtime = bazel(
        &root,
        "test",
        &["//bad:missing_runtime_test"],
        &["--test_output=all"],
    );
    fails_with(
        &runtime,
        "dx-apple-test-simulator: no available iOS 17.9 simulator runtime com.apple.CoreSimulator.SimRuntime.iOS-17-9",
    );
    assert!(!passed(&runtime, "testNothing"), "{}", runtime.combined());
    assert!(
        !runtime.combined().contains("Selected simulator runtime"),
        "{}",
        runtime.combined()
    );
    let devices = tool("xcrun", &["simctl", "list", "devices"]);
    assert!(!devices.contains("dx iPhone 16 iOS 17.9"), "{devices}");
}
