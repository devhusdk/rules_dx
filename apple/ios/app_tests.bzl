"""Simulator application, app test, and XCFramework selection coverage."""

load("//apple/bundle:defs.bzl", "bundle_id_errors", "bundle_version_errors")
load("//libs/starlark:defs.bzl", "expect_contains", "expect_equal", "starlark_test")
load(
    ":app.bzl",
    "app_errors",
    "app_resource_errors",
    "default_app",
    "default_framework",
    "entitlement_errors",
    "entry_errors",
    "framework_errors",
    "framework_header_errors",
    "framework_header_name_errors",
    "framework_resource_errors",
    "signing_errors",
    "slice_errors",
    "test_errors",
    "test_source_errors",
)
load(":simulator.bzl", "simulator_errors")

def _app(**kwargs):
    base = default_app(kwargs.get("environment", "simulator"))
    fields = [
        "arch",
        "bundle_id",
        "deployment",
        "entitlements",
        "entry",
        "environment",
        "frameworks",
        "library",
        "provisioning_profile",
        "resources",
        "srcs",
        "version",
        "xcode_version",
    ]
    values = {key: kwargs.get(key, getattr(base, key)) for key in fields}
    if "bundle_id" not in kwargs:
        values["bundle_id"] = "com.example.share"
    if "version" not in kwargs:
        values["version"] = "1.0"
    if "library" not in kwargs:
        values["library"] = "//share:share_rust"
    if "srcs" not in kwargs:
        values["srcs"] = ["main.m"]
    if "deployment" not in kwargs:
        values["deployment"] = "17.0"
    if "xcode_version" not in kwargs:
        values["xcode_version"] = "16.2"
    return struct(**values)

def _framework(**kwargs):
    base = default_framework()
    fields = ["bundle_id", "bundle_name", "deployment", "hdrs", "library", "resources", "slices", "version", "xcode_version"]
    values = {key: kwargs.get(key, getattr(base, key)) for key in fields}
    defaults = {
        "bundle_id": "com.example.share",
        "bundle_name": "Share",
        "deployment": "17.0",
        "hdrs": ["share_api.h"],
        "library": "//share:share_rust",
        "slices": ["device/arm64", "simulator/arm64"],
        "version": "1.0",
        "xcode_version": "16.4",
    }
    for key, value in defaults.items():
        if key not in kwargs:
            values[key] = value
    return struct(**values)

def _app_checks():
    checks = []
    checks.append(expect_equal("empty app bundle id fails", bundle_id_errors("apple_app", ""), ["apple_app: bundle_id must not be empty"]))
    checks.append(expect_equal(
        "flat app bundle id fails",
        bundle_id_errors("apple_app", "share"),
        ["apple_app: bundle_id 'share' is not reverse-DNS: want like 'com.example.app'"],
    ))
    checks.append(expect_equal(
        "dotted app bundle id with empty part fails",
        bundle_id_errors("apple_app", "com..app"),
        ["apple_app: bundle_id 'com..app' is not reverse-DNS: want like 'com.example.app'"],
    ))
    checks.append(expect_equal(
        "app bundle id with space fails",
        bundle_id_errors("apple_app", "com.exa mple.app"),
        ["apple_app: bundle_id 'com.exa mple.app' uses ' ': want letters, digits, '-', '_', '.'"],
    ))
    checks.append(expect_equal("reverse-DNS app bundle id is clean", bundle_id_errors("apple_app", "com.example.share"), []))
    checks.append(expect_equal("framework kind names the owner", bundle_id_errors("apple_framework", ""), ["apple_framework: bundle_id must not be empty"]))
    checks.append(expect_equal("empty bundle version fails", bundle_version_errors("apple_app", ""), ["apple_app: bundle version must not be empty"]))
    checks.append(expect_equal(
        "freeform bundle version fails",
        bundle_version_errors("apple_app", "one"),
        ["apple_app: bundle version 'one' is not a dotted version"],
    ))
    checks.append(expect_equal("dotted bundle version is clean", bundle_version_errors("apple_app", "1.0"), []))
    checks.append(expect_equal("no app resources are clean", app_resource_errors([]), []))
    checks.append(expect_equal("named app resources are clean", app_resource_errors(["Assets.xcassets"]), []))
    checks.append(expect_equal(
        "empty app resource fails",
        app_resource_errors(["Assets.xcassets", ""]),
        ["apple_app: resource names must not be empty"],
    ))
    checks.append(expect_equal("empty entitlements are clean", entitlement_errors(""), []))
    checks.append(expect_equal("entitlements file is clean", entitlement_errors("Share.entitlements"), []))
    checks.append(expect_equal(
        "plist entitlements fail",
        entitlement_errors("Share.plist"),
        ["apple_app: entitlements 'Share.plist' must be an .entitlements file"],
    ))
    checks.append(expect_equal("unsigned simulator is clean", signing_errors("simulator", ""), []))
    checks.append(expect_equal(
        "provisioned simulator fails",
        signing_errors("simulator", "share.mobileprovision"),
        ["apple_app: simulator builds sign ad hoc; leave provisioning_profile empty"],
    ))
    checks.append(expect_equal(
        "unprovisioned device fails",
        signing_errors("device", ""),
        ["apple_app: device builds need provisioning_profile, a .mobileprovision file"],
    ))
    checks.append(expect_equal("provisioned device is clean", signing_errors("device", "share.mobileprovision"), []))
    checks.append(expect_equal(
        "plist profile on device fails",
        signing_errors("device", "share.plist"),
        ["apple_app: provisioning_profile 'share.plist' must be a .mobileprovision file"],
    ))
    checks.append(expect_equal(
        "profile label on device fails",
        signing_errors("device", "//share:profile"),
        ["apple_app: provisioning_profile '//share:profile' must be a .mobileprovision file"],
    ))
    checks.append(expect_equal(
        "app without entry source fails",
        entry_errors([], ""),
        ["apple_app: srcs needs the native entry point that calls UIApplicationMain, or entry names a Swift @main module"],
    ))
    checks.append(expect_equal(
        "app with srcs and entry fails",
        entry_errors(["main.m"], "//app:main"),
        ["apple_app: set srcs or entry for the app entry point, not both"],
    ))
    checks.append(expect_equal("app entry source is clean", entry_errors(["main.m"], ""), []))
    checks.append(expect_equal("app entry module is clean", entry_errors([], "//app:main"), []))
    checks.append(expect_equal("test without sources fails", test_source_errors([], []), ["apple_app_test: srcs or deps needs the XCTest cases"]))
    checks.append(expect_equal("test with srcs is clean", test_source_errors(["t.m"], []), []))
    checks.append(expect_equal("test with deps is clean", test_source_errors([], ["//app:tests"]), []))
    checks.append(expect_equal(
        "entry alone composes the app",
        app_errors(_app(library = "", srcs = [], entry = "//app:main")),
        [],
    ))
    checks.append(expect_equal("clean simulator app", app_errors(_app()), []))
    checks.append(expect_equal(
        "clean signed device app",
        app_errors(_app(
            environment = "device",
            provisioning_profile = "share.mobileprovision",
        )),
        [],
    ))
    checks.append(expect_contains(
        "missing bundle id fails the app",
        app_errors(_app(bundle_id = "")),
        "apple_app: bundle_id must not be empty",
    ))
    checks.append(expect_contains(
        "missing library fails the app",
        app_errors(_app(library = "")),
        "apple_app: app composes one qualified Rust library cell in library or XCFramework imports in frameworks, or a Swift @main module in entry",
    ))
    checks.append(expect_equal(
        "framework imports alone compose the app",
        app_errors(_app(library = "", frameworks = ["//vendor:share"])),
        [],
    ))
    checks.append(expect_contains(
        "unsigned device fails the app",
        app_errors(_app(environment = "device")),
        "apple_app: device builds need provisioning_profile, a .mobileprovision file",
    ))
    checks.append(expect_contains(
        "plist profile fails the device app",
        app_errors(_app(environment = "device", provisioning_profile = "share.plist")),
        "apple_app: provisioning_profile 'share.plist' must be a .mobileprovision file",
    ))
    checks.append(expect_contains(
        "missing deployment fails the app",
        app_errors(_app(deployment = "")),
        "apple_app: deployment names the minimum iOS version",
    ))
    checks.append(expect_contains(
        "deployment above the Xcode 16 ceiling fails the app",
        app_errors(_app(deployment = "19.0")),
        "apple_ios: deployment_target '19.0' is above the Xcode 16 'iphonesimulator' ceiling 18.0",
    ))
    checks.append(expect_contains(
        "missing entry fails the app",
        app_errors(_app(srcs = [])),
        "apple_app: srcs needs the native entry point that calls UIApplicationMain, or entry names a Swift @main module",
    ))
    checks.append(expect_contains(
        "device x86_64 names the simulator cell",
        app_errors(_app(environment = "device", arch = "x86_64")),
        "apple_ios: device builds need arch 'arm64'; 'x86_64' runs on the simulator",
    ))
    checks.append(expect_equal(
        "no slices fail",
        slice_errors([]),
        ["apple_framework: framework needs at least one 'environment/arch' slice"],
    ))
    checks.append(expect_equal(
        "flat slice fails",
        slice_errors(["simulator"]),
        [
            "apple_framework: slice 'simulator' is not 'environment/arch'",
            "apple_framework: framework needs the device/arm64 slice",
            "apple_framework: framework needs at least one simulator slice",
        ],
    ))
    checks.append(expect_equal(
        "unknown slice environment fails",
        slice_errors(["watch/arm64"]),
        [
            "apple_framework: unknown environment 'watch' in slice 'watch/arm64': want device, simulator",
            "apple_framework: framework needs the device/arm64 slice",
            "apple_framework: framework needs at least one simulator slice",
        ],
    ))
    checks.append(expect_equal(
        "device simulator arch fails",
        slice_errors(["device/x86_64", "simulator/arm64"]),
        [
            "apple_framework: arch 'x86_64' is outside device in slice 'device/x86_64': want arm64",
            "apple_framework: framework needs the device/arm64 slice",
        ],
    ))
    checks.append(expect_equal(
        "duplicate slices fail",
        slice_errors(["device/arm64", "simulator/arm64", "simulator/arm64"]),
        ["apple_framework: duplicate slice 'simulator/arm64'"],
    ))
    checks.append(expect_equal(
        "device-only slices fail",
        slice_errors(["device/arm64"]),
        ["apple_framework: framework needs at least one simulator slice"],
    ))
    checks.append(expect_equal(
        "simulator-only slices fail",
        slice_errors(["simulator/arm64", "simulator/x86_64"]),
        ["apple_framework: framework needs the device/arm64 slice"],
    ))
    checks.append(expect_equal(
        "device plus simulator slices are clean",
        slice_errors(["device/arm64", "simulator/arm64", "simulator/x86_64"]),
        [],
    ))
    checks.append(expect_equal("no framework resources are clean", framework_resource_errors([]), []))
    checks.append(expect_equal(
        "absolute framework resource fails",
        framework_resource_errors(["/abs/Assets.xcassets"]),
        ["apple_framework: resource '/abs/Assets.xcassets' must be a relative workspace path"],
    ))
    checks.append(expect_equal(
        "empty framework resource fails",
        framework_resource_errors([""]),
        ["apple_framework: resource names must not be empty"],
    ))
    checks.append(expect_equal("clean framework", framework_errors(_framework()), []))
    checks.append(expect_contains(
        "missing framework bundle id fails",
        framework_errors(_framework(bundle_id = "")),
        "apple_framework: bundle_id must not be empty",
    ))
    checks.append(expect_equal("one public header is clean", framework_header_name_errors(["share_api.h"], "Share"), []))
    checks.append(expect_equal(
        "no public headers fail",
        framework_header_errors([]),
        ["apple_framework: hdrs needs at least one public header for the framework module map"],
    ))
    checks.append(expect_equal(
        "non-header public file fails",
        framework_header_name_errors(["share.m"], "Share"),
        ["apple_framework: public header 'share.m' must be a .h file"],
    ))
    checks.append(expect_equal(
        "public header named like the umbrella fails on case-insensitive volumes",
        framework_header_name_errors(["share.h"], "Share"),
        ["apple_framework: public header 'share.h' clashes with the umbrella header Share.h"],
    ))
    checks.append(expect_contains(
        "missing framework library fails",
        framework_errors(_framework(library = "")),
        "apple_framework: framework composes one Rust static library in library",
    ))
    checks.append(expect_contains(
        "missing framework deployment fails",
        framework_errors(_framework(deployment = "")),
        "apple_framework: deployment names the minimum iOS version",
    ))
    checks.append(expect_contains(
        "framework deployment above the Xcode 16 ceiling fails",
        framework_errors(_framework(deployment = "18.1")),
        "apple_ios: deployment_target '18.1' is above the Xcode 16 'iphoneos' ceiling 18.0",
    ))
    checks.append(expect_contains(
        "framework deployment below the floor fails",
        framework_errors(_framework(deployment = "16.4")),
        "apple_ios: deployment_target '16.4' is below 17.0",
    ))
    checks.append(expect_contains(
        "framework Xcode outside the lineage fails",
        framework_errors(_framework(xcode_version = "15.4")),
        "apple_ios: xcode '15.4' is outside 16",
    ))
    checks.append(expect_contains(
        "duplicate framework slice fails",
        framework_errors(_framework(slices = ["device/arm64", "simulator/arm64", "device/arm64"])),
        "apple_framework: duplicate slice 'device/arm64'",
    ))
    checks.append(expect_contains(
        "wrong framework slice fails",
        framework_errors(_framework(slices = ["device/arm64", "simulator/armv7"])),
        "apple_framework: arch 'armv7' is outside simulator in slice 'simulator/armv7': want arm64, x86_64",
    ))
    checks.append(expect_equal("declared simulator test is clean", test_errors("iPhone 16", "18.5", "17.0"), []))
    checks.append(expect_equal(
        "undeclared simulator test fails",
        test_errors("", "", ""),
        [
            "apple_app_test: device_type names the simulator device, like 'iPhone 16'",
            "apple_app_test: os_version names the installed simulator runtime, like '18.5'",
            "apple_app_test: deployment names the minimum iOS version of the host app",
        ],
    ))
    checks.append(expect_equal(
        "freeform runtime fails",
        test_errors("iPhone 16", "latest", "17.0"),
        ["apple_app_test: os_version 'latest' is not a dotted version"],
    ))
    checks.append(expect_equal(
        "test deployment below the floor fails",
        test_errors("iPhone 16", "18.5", "16.0"),
        ["apple_ios: deployment_target '16.0' is below 17.0"],
    ))
    checks.append(expect_equal(
        "runtime below the app deployment fails",
        test_errors("iPhone 16", "17.4", "18.0"),
        ["apple_app_test: simulator runtime iOS 17.4 is below the app deployment 18.0"],
    ))
    checks.append(expect_equal("named simulator is clean", simulator_errors("iPhone 16"), []))
    checks.append(expect_equal(
        "unnamed simulator fails",
        simulator_errors(""),
        ["apple_simulator: simulator names the target simulator by name or UDID"],
    ))
    return checks

def apple_ios_app_tests(name):
    """Declares the simulator app and framework inventory pin test."""
    starlark_test(
        name = name,
        mode = "load",
        checks = _app_checks(),
    )
