mod consumer;
mod macos;
mod signing;

use std::path::{Path, PathBuf};

use consumer::{consumer, fails_with, read, write};
use dx_nested::Case;
use dx_testing::mkscratch;
use macos::{
    bazel, installed_xcode, keep, passed, passes, runtime_for, tool, version, wait_for, Simulator,
};
use signing::profile;

/// Upstream rules_swift, resolved by the consumer and renamed so the test
/// proves `entry` and test `deps` accept any CcInfo or SwiftInfo target.
const RULES_SWIFT: &str = "4.1.1";

const MODULE_EXTRA: &str = r#"
bazel_dep(name = "rules_rust", version = "0.74.0")
bazel_dep(name = "rules_swift", version = "RULES_SWIFT", repo_name = "swift_rules")

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
load("@rules_cc//cc:defs.bzl", "cc_import", "cc_library")
load("@rules_rust//rust:defs.bzl", "rust_static_library")
load("@swift_rules//swift:swift.bzl", "swift_binary", "swift_library", "swift_test")
load("@swift_rules//swift:swift_interop_hint.bzl", "swift_interop_hint")

rust_static_library(
    name = "ledger_rust",
    srcs = ["ledger.rs"],
    edition = "2021",
)

swift_interop_hint(
    name = "ledger_core_hint",
    module_name = "LedgerCore",
)

cc_library(
    name = "ledger_core",
    hdrs = ["ledger.h"],
    aspect_hints = [":ledger_core_hint"],
)

swift_library(
    name = "ledger_kit",
    srcs = ["LedgerKit.swift"],
    module_name = "LedgerKit",
)

swift_library(
    name = "ledger_main",
    srcs = ["LedgerAppDelegate.swift"],
    module_name = "LedgerApp",
    deps = [
        ":ledger_core",
        ":ledger_kit",
    ],
)

swift_library(
    name = "ledger_xctests",
    testonly = True,
    srcs = ["LedgerAppTests.swift"],
    module_name = "LedgerAppTests",
    deps = [
        ":ledger_core",
        ":ledger_kit",
    ],
)

swift_binary(
    name = "ledger_cli",
    srcs = ["cli/main.swift"],
    deps = [":ledger_kit"],
)

swift_test(
    name = "ledger_kit_tests",
    srcs = ["LedgerKitTests.swift"],
    deps = [":ledger_kit"],
)

apple_ios_app(
    name = "ledger_app",
    arch = "arm64",
    bundle_id = "com.example.ledger",
    deployment = "17.0",
    entry = ":ledger_main",
    environment = "simulator",
    library = ":ledger_rust",
    resources = ["entries.txt"],
    version = "1.4",
    xcode_version = "XCODE",
)

apple_ios_app_test(
    name = "ledger_app_test",
    size = "large",
    app = ":ledger_app",
    deployment = "17.0",
    device_type = "iPhone 16",
    os_version = "RUNTIME",
    deps = [":ledger_xctests"],
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

cc_import(
    name = "device_lib",
    static_library = "libdevice.a",
)

swift_library(
    name = "device_main",
    srcs = ["DeviceMain.swift"],
    module_name = "DeviceMain",
    deps = [":ledger_kit"],
)

apple_ios_app(
    name = "device_app",
    bundle_id = "com.example.device",
    deployment = "17.0",
    entry = ":device_main",
    environment = "device",
    library = ":device_lib",
    provisioning_profile = "dev.mobileprovision",
    version = "1.0",
)

apple_ios_app(
    name = "unsigned_device_app",
    bundle_id = "com.example.device",
    deployment = "17.0",
    entry = ":device_main",
    environment = "device",
    library = ":device_lib",
    version = "1.0",
)

swift_library(
    name = "broken_main",
    srcs = ["Broken.swift"],
    module_name = "Broken",
)

apple_ios_app(
    name = "broken_app",
    bundle_id = "com.example.broken",
    deployment = "17.0",
    entry = ":broken_main",
    environment = "simulator",
    library = ":ledger_rust",
    version = "1.0",
    xcode_version = "XCODE",
)

apple_ios_app(
    name = "both_app",
    srcs = ["unused.m"],
    bundle_id = "com.example.both",
    deployment = "17.0",
    entry = ":ledger_main",
    environment = "simulator",
    library = ":ledger_rust",
    version = "1.0",
)

apple_ios_app(
    name = "wrong_xcode_app",
    bundle_id = "com.example.wrong",
    deployment = "17.0",
    entry = ":ledger_main",
    environment = "simulator",
    library = ":ledger_rust",
    version = "1.0",
    xcode_version = "9.9",
)

swift_library(
    name = "failing_xctests",
    testonly = True,
    srcs = ["FailingTests.swift"],
    module_name = "FailingTests",
)

apple_ios_app_test(
    name = "failing_app_test",
    size = "large",
    app = ":ledger_app",
    deployment = "17.0",
    device_type = "iPhone 16",
    os_version = "RUNTIME",
    deps = [":failing_xctests"],
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

const LEDGER_KIT_SWIFT: &str = r#"import Foundation

public struct Ledger {
  public private(set) var cents: [Int64] = []

  public init?(text: String) {
    for line in text.split(separator: "\n") {
      let trimmed = line.trimmingCharacters(in: .whitespaces)
      if trimmed.isEmpty {
        continue
      }
      let fields = trimmed.split(separator: " ")
      guard fields.count >= 2, let value = Int64(fields[fields.count - 1]) else {
        return nil
      }
      cents.append(value)
    }
  }

  public var total: Int64 {
    return cents.reduce(0, +)
  }

  public mutating func add(cents value: Int64) {
    cents.append(value)
  }

  public func state(platform: String, total: Int64) -> String {
    return "platform=\(platform)\ntotal=\(total)\n"
  }
}
"#;

const DELEGATE_SWIFT: &str = r#"import Foundation
import LedgerCore
import LedgerKit
import UIKit

@main
final class LedgerAppDelegate: UIResponder, UIApplicationDelegate {
  var window: UIWindow?

  func application(
    _ application: UIApplication,
    didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]? = nil
  ) -> Bool {
    let root = UIViewController()
    let label = UILabel(frame: CGRect(x: 20, y: 80, width: 280, height: 40))
    root.view.addSubview(label)
    let window = UIWindow(frame: UIScreen.main.bounds)
    window.rootViewController = root
    window.makeKeyAndVisible()
    self.window = window

    guard let path = Bundle.main.path(forResource: "entries", ofType: "txt"),
      let text = try? String(contentsOfFile: path, encoding: .utf8),
      let ledger = Ledger(text: text)
    else {
      return true
    }
    let total = ledger_total(text)
    label.text = "\(total) cents"
    guard total == ledger.total else {
      return true
    }
    let documents = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask)[0]
    let state = ledger.state(platform: String(cString: ledger_platform()), total: total)
    try? state.write(
      to: documents.appendingPathComponent("ledger-state.txt"), atomically: true, encoding: .utf8)
    return true
  }
}
"#;

const ENTRIES: &str = "tea 450\nbread 325\njam 600\n";

const TESTS_SWIFT: &str = r#"import Foundation
import LedgerCore
import LedgerKit
import UIKit
import XCTest

final class LedgerAppTests: XCTestCase {
  func testRustTotalsLedgerLines() {
    XCTAssertEqual(ledger_total("tea 450\nbread 325\n"), 775)
    XCTAssertEqual(ledger_total("broken"), -1)
  }

  func testRustRunsOnTheSimulator() {
    XCTAssertEqual(String(cString: ledger_platform()), "iphonesimulator")
  }

  func testHostAppLoadedTheBundledEntries() throws {
    let delegate = try XCTUnwrap(UIApplication.shared.delegate)
    XCTAssertTrue(NSStringFromClass(type(of: delegate)).contains("LedgerAppDelegate"))
    let path = try XCTUnwrap(Bundle.main.path(forResource: "entries", ofType: "txt"))
    let text = try String(contentsOfFile: path, encoding: .utf8)
    XCTAssertEqual(ledger_total(text), 1375)
    XCTAssertEqual(Ledger(text: text)?.total, 1375)
  }

  func testSwiftAndRustAgreeOnAnAddedEntry() throws {
    let path = try XCTUnwrap(Bundle.main.path(forResource: "entries", ofType: "txt"))
    let text = try String(contentsOfFile: path, encoding: .utf8)
    var ledger = try XCTUnwrap(Ledger(text: text))
    ledger.add(cents: 500)
    XCTAssertEqual(ledger.total, 1875)
    XCTAssertEqual(ledger_total(text + "cake 500\n"), 1875)
    XCTAssertEqual(
      ledger.state(platform: "iphonesimulator", total: ledger.total),
      "platform=iphonesimulator\ntotal=1875\n")
  }
}
"#;

const CLI_SWIFT: &str = r#"import Foundation
import LedgerKit

guard let ledger = Ledger(text: "tea 450\nbread 325\njam 600\n") else {
  print("ledger: malformed")
  exit(1)
}
print("total=\(ledger.total)")
"#;

const KIT_TESTS_SWIFT: &str = r#"import LedgerKit
import XCTest

final class LedgerKitTests: XCTestCase {
  func testTotalsEntries() {
    XCTAssertEqual(Ledger(text: "tea 450\nbread 325\n")?.total, 775)
  }

  func testRejectsMalformedLines() {
    XCTAssertNil(Ledger(text: "broken"))
  }

  func testAddingAnEntryChangesTheState() throws {
    var ledger = try XCTUnwrap(Ledger(text: "tea 450\n"))
    ledger.add(cents: 500)
    XCTAssertEqual(
      ledger.state(platform: "macos", total: ledger.total), "platform=macos\ntotal=950\n")
  }
}
"#;

const DEVICE_MAIN_SWIFT: &str = r#"import LedgerKit
import UIKit

@main
final class DeviceAppDelegate: UIResponder, UIApplicationDelegate {
  var window: UIWindow?

  func application(
    _ application: UIApplication,
    didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]? = nil
  ) -> Bool {
    return Ledger(text: "tea 450\n") != nil
  }
}
"#;

const BROKEN_SWIFT: &str = r#"import UIKit

@main
final class BrokenDelegate: UIResponder, UIApplicationDelegate {
  let value: Int = "not a number"
}
"#;

const FAILING_TESTS_SWIFT: &str = r#"import XCTest

final class FailingTests: XCTestCase {
  func testStillPasses() {
    XCTAssertTrue(true)
  }

  func testSwiftCaseFails() {
    XCTAssertEqual(1 + 1, 3, "swift cases really run")
  }
}
"#;

const UNUSED_M: &str = "int ledger_unused(void) { return 0; }\n";

const MARKER_C: &str = "int ledger_marker(void) { return 7; }\n";

fn ledger(xcode: &str, runtime: &str, simulator: &str) -> Case {
    let root = consumer(
        "apple-swift-consumer",
        &MODULE_EXTRA.replace("RULES_SWIFT", RULES_SWIFT),
        BAZELRC,
    );
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
    write(&root.join("ledger/LedgerKit.swift"), LEDGER_KIT_SWIFT);
    write(&root.join("ledger/LedgerAppDelegate.swift"), DELEGATE_SWIFT);
    write(&root.join("ledger/LedgerAppTests.swift"), TESTS_SWIFT);
    write(&root.join("ledger/LedgerKitTests.swift"), KIT_TESTS_SWIFT);
    write(&root.join("ledger/cli/main.swift"), CLI_SWIFT);
    write(&root.join("ledger/entries.txt"), ENTRIES);
    write(&root.join("ledger/DeviceMain.swift"), DEVICE_MAIN_SWIFT);
    write(&root.join("ledger/Broken.swift"), BROKEN_SWIFT);
    write(&root.join("ledger/FailingTests.swift"), FAILING_TESTS_SWIFT);
    write(&root.join("ledger/unused.m"), UNUSED_M);
    root
}

fn device_archive(root: &Path) {
    let dir = root.join("ledger");
    let source = dir.join("marker.c");
    write(&source, MARKER_C);
    let object = dir.join("device.o");
    tool(
        "xcrun",
        &[
            "--sdk",
            "iphoneos",
            "clang",
            "-target",
            "arm64-apple-ios17.0",
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
            &dir.join("libdevice.a").to_string_lossy(),
            &object.to_string_lossy(),
        ],
    );
    std::fs::remove_file(&object).unwrap_or_else(|error| panic!("{object:?}: {error}"));
}

fn unpack(ipa: &Path, app: &str) -> PathBuf {
    let dir = mkscratch("apple-swift-ipa").unwrap_or_else(|error| panic!("scratch: {error}"));
    tool(
        "unzip",
        &["-q", &ipa.to_string_lossy(), "-d", &dir.to_string_lossy()],
    );
    dir.join(format!("Payload/{app}.app"))
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
fn a_swift_app_builds_runs_and_tests_on_macos_and_a_simulator() {
    let xcode = installed_xcode();
    let sdk_version = tool("xcrun", &["--sdk", "iphonesimulator", "--show-sdk-version"]);
    let runtime = runtime_for(&sdk_version);
    assert!(
        version(&runtime) >= version("17.0"),
        "the simulator runtime {runtime} is older than the app's iOS 17.0 deployment"
    );
    let simulator = Simulator::create("dx-swift", &runtime);
    let root = ledger(&xcode, &runtime, &simulator.name);

    let cli = bazel(&root, "run", &["//ledger:ledger_cli"], &[]);
    passes(&cli);
    assert!(cli.combined().contains("total=1375"), "{}", cli.combined());

    let native = bazel(
        &root,
        "test",
        &["//ledger:ledger_kit_tests"],
        &["--test_output=all"],
    );
    passes(&native);
    for case in [
        "testTotalsEntries",
        "testRejectsMalformedLines",
        "testAddingAnEntryChangesTheState",
    ] {
        assert!(
            passed(&native, case),
            "{case} did not pass:\n{}",
            native.combined()
        );
    }

    passes(&bazel(&root, "build", &["//ledger:ledger_app"], &[]));
    let app = unpack(&root.join("bazel-bin/ledger/ledger_app.ipa"), "ledger_app");
    assert_eq!(plist(&app, "CFBundleIdentifier"), BUNDLE_ID);
    assert_eq!(plist(&app, "CFBundleExecutable"), "ledger_app");
    assert_eq!(plist(&app, "MinimumOSVersion"), "17.0");
    assert_eq!(plist(&app, "DTPlatformName"), "iphonesimulator");
    assert_eq!(read(&app.join("entries.txt")), ENTRIES);
    let binary = app.join("ledger_app").to_string_lossy().into_owned();
    assert_eq!(tool("xcrun", &["lipo", "-archs", &binary]), "arm64");
    let loads = tool("xcrun", &["otool", "-l", &binary]);
    keep("ledger_app.otool.txt", &loads);
    assert!(loads.contains("platform 7"), "{loads}");
    assert!(loads.contains("minos 17.0"), "{loads}");
    let libraries = tool("xcrun", &["otool", "-L", &binary]);
    assert!(libraries.contains("libswiftCore"), "{libraries}");
    let symbols = tool("xcrun", &["nm", "-gU", &binary]);
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
        "testSwiftAndRustAgreeOnAnAddedEntry",
    ] {
        assert!(
            passed(&tested, case),
            "{case} did not pass:\n{}",
            tested.combined()
        );
    }
    let leaked: Vec<String> = dx_owned_simulators()
        .into_iter()
        .filter(|name| !owned_before.contains(name))
        .collect();
    assert!(
        leaked.is_empty(),
        "the test simulator outlived its run: {leaked:?}"
    );

    passes(&bazel(&root, "run", &["//ledger:boot"], &[]));
    passes(&bazel(&root, "run", &["//ledger:install"], &[]));
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

    let swift = tool("xcrun", &["swift", "-version"]);
    let arch = tool("uname", &["-m"]);
    let facts = format!(
        "rules_swift={RULES_SWIFT}\nswift={}\nxcode={xcode}\nsdk=iphonesimulator\nsdk_version={sdk_version}\nruntime=iOS {runtime}\ndevice_type=iPhone 16\nsimulator={}\nhost_arch={arch}\napp_arch=arm64\nstate={}\n",
        swift.replace('\n', ";"),
        simulator.udid,
        state.replace('\n', ";")
    );
    keep("swift-facts.txt", &facts);
}

#[test]
fn a_swift_device_app_signs_ad_hoc_with_its_declared_profile() {
    let xcode = installed_xcode();
    let root = ledger(&xcode, "18.0", "unused");
    device_archive(&root);
    profile(&root.join("ledger"), "dev", "2099-01-01T00:00:00Z");

    let built = bazel(
        &root,
        "build",
        &["//ledger:device_app"],
        &[
            "--ios_multi_cpus=arm64",
            "--@rules_apple//apple/build_settings:signing_certificate_name=Apple Development",
        ],
    );
    passes(&built);
    let ipa = root.join("bazel-bin/ledger/device_app.ipa");
    assert!(ipa.is_file(), "{}", built.combined());
    let app = unpack(&ipa, "device_app");
    assert_eq!(
        std::fs::read(app.join("embedded.mobileprovision")).ok(),
        std::fs::read(root.join("ledger/dev.mobileprovision")).ok(),
        "the declared profile is embedded"
    );
    let signature = std::process::Command::new("codesign")
        .args(["-dv", &app.to_string_lossy()])
        .output()
        .unwrap_or_else(|error| panic!("codesign: {error}"));
    let details = String::from_utf8_lossy(&signature.stderr);
    assert!(details.contains("Signature=adhoc"), "{details}");

    let unsigned = bazel(
        &root,
        "build",
        &["//ledger:unsigned_device_app"],
        &["--ios_multi_cpus=arm64"],
    );
    fails_with(
        &unsigned,
        "apple_app: device builds need provisioning_profile, a .mobileprovision file",
    );
}

#[test]
fn swift_failures_fail_before_success() {
    let xcode = installed_xcode();
    let sdk_version = tool("xcrun", &["--sdk", "iphonesimulator", "--show-sdk-version"]);
    let runtime = runtime_for(&sdk_version);
    let root = ledger(&xcode, &runtime, "unused");

    let broken = bazel(&root, "build", &["//ledger:broken_app"], &[]);
    fails_with(&broken, "cannot convert value of type");
    assert!(
        !root.join("bazel-bin/ledger/broken_app.ipa").exists(),
        "a Swift compile error still produced an app"
    );

    let both = bazel(&root, "build", &["//ledger:both_app"], &[]);
    fails_with(&both, "set srcs or entry for the app entry point, not both");

    let wrong = bazel(&root, "build", &["//ledger:wrong_xcode_app"], &[]);
    fails_with(&wrong, "selects Xcode 9.9 but Bazel resolved");
    assert!(
        !root.join("bazel-bin/ledger/wrong_xcode_app.ipa").exists(),
        "a mismatched Xcode still produced an app"
    );

    let failing = bazel(
        &root,
        "test",
        &["//ledger:failing_app_test"],
        &["--test_output=all"],
    );
    fails_with(&failing, "testSwiftCaseFails");
    assert!(
        passed(&failing, "testStillPasses"),
        "the passing Swift case did not run:\n{}",
        failing.combined()
    );
    assert!(
        !passed(&failing, "testSwiftCaseFails"),
        "{}",
        failing.combined()
    );
    assert!(
        failing
            .combined()
            .lines()
            .any(|line| line.contains("testSwiftCaseFails") && line.contains("' failed")),
        "{}",
        failing.combined()
    );
}
