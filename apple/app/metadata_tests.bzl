"""Composed iOS app bundle and framework pin coverage."""

load("//libs/starlark:defs.bzl", "expect_equal", "expect_match", "expect_true", "starlark_test")
load(
    ":defs.bzl",
    "app_errors",
    "app_file_name",
    "app_version_errors",
    "bundle_id_errors",
    "default_app",
    "default_framework",
    "effects",
    "entitlement_errors",
    "framework_file_name",
    "operation_errors",
    "operations",
    "pending_errors",
    "resource_errors",
    "slice_errors",
    "xcframework_errors",
    "xcframework_file_name",
)

def _app(**kwargs):
    base = default_app(kwargs.get("environment", "simulator"))
    fields = [
        "allow_effects",
        "arch",
        "bundle_id",
        "deployment",
        "entitlements",
        "environment",
        "frameworks",
        "operation",
        "provisioning_profile",
        "resources",
        "sdk",
        "triple",
        "version",
        "xcode_version",
    ]
    values = {key: kwargs.get(key, getattr(base, key)) for key in fields}
    if "arch" not in kwargs:
        values["arch"] = "arm64"
    if "bundle_id" not in kwargs:
        values["bundle_id"] = "com.example.share"
    if "deployment" not in kwargs:
        values["deployment"] = "17.0"
    if "frameworks" not in kwargs:
        values["frameworks"] = ["Foundation"]
    if "resources" not in kwargs:
        values["resources"] = ["share.dat"]
    if "version" not in kwargs:
        values["version"] = "1.0"
    if "xcode_version" not in kwargs:
        values["xcode_version"] = "16.2"
    return struct(**values)

def _framework(**kwargs):
    base = default_framework()
    values = {
        "name": kwargs.get("name", base.name),
        "served": kwargs.get("served", base.served),
        "slices": kwargs.get("slices", base.slices),
    }
    if "name" not in kwargs:
        values["name"] = "share"
    if "served" not in kwargs:
        values["served"] = ["simulator"]
    if "slices" not in kwargs:
        values["slices"] = ["simulator/arm64"]
    return struct(**values)

def _app_checks():
    checks = []
    checks.append(expect_equal("operations", operations(), ["build", "export", "launch", "publish", "sign"]))
    checks.append(expect_equal("effects", effects(), ["export", "publish", "sign"]))
    checks.append(expect_equal("app file name", app_file_name("share"), "share.app"))
    checks.append(expect_equal("framework file name", framework_file_name("share"), "share.framework"))
    checks.append(expect_equal("xcframework file name", xcframework_file_name("share"), "share.xcframework"))
    checks.append(expect_equal(
        "missing bundle identifier fails",
        bundle_id_errors(""),
        ["apple_app: select one bundle identifier: want reverse-DNS 'com.example.app'"],
    ))
    checks.append(expect_equal("dotted bundle identifier is clean", bundle_id_errors("com.example.share"), []))
    checks.append(expect_equal(
        "flat bundle identifier fails",
        bundle_id_errors("share"),
        ["apple_app: bundle identifier 'share' is not reverse-DNS"],
    ))
    checks.append(expect_equal(
        "gapped bundle identifier fails",
        bundle_id_errors("com..share"),
        ["apple_app: bundle identifier 'com..share' is not reverse-DNS"],
    ))
    checks.append(expect_equal("empty version is clean", app_version_errors(""), []))
    checks.append(expect_equal("dotted version is clean", app_version_errors("1.0"), []))
    checks.append(expect_equal(
        "freeform version fails",
        app_version_errors("one"),
        ["apple_app: version 'one' is not a dotted version"],
    ))
    checks.append(expect_equal("no resources are clean", resource_errors([]), []))
    checks.append(expect_equal("bundled resources are clean", resource_errors(["share.dat", "Assets/icon.png"]), []))
    checks.append(expect_equal(
        "empty resource fails",
        resource_errors(["share.dat", ""]),
        ["apple_app: resource names must not be empty"],
    ))
    checks.append(expect_equal(
        "escaping resource fails",
        resource_errors(["../share.dat"]),
        ["apple_app: resource '../share.dat' escapes the bundle"],
    ))
    checks.append(expect_equal("no entitlements are clean", entitlement_errors([]), []))
    checks.append(expect_equal(
        "duplicated entitlement fails",
        entitlement_errors(["applinks", "applinks"]),
        ["apple_app: entitlement 'applinks' is listed twice"],
    ))
    checks.append(expect_equal(
        "empty entitlement fails",
        entitlement_errors(["applinks", ""]),
        ["apple_app: entitlement names must not be empty"],
    ))
    checks.append(expect_equal(
        "simulator slice is clean",
        slice_errors(["simulator"], ["simulator/arm64"]),
        [],
    ))
    checks.append(expect_equal(
        "malformed slice fails",
        slice_errors(["simulator"], ["simulator"]),
        ["apple_app: slice 'simulator' is not 'environment/arch'"],
    ))
    checks.append(expect_equal(
        "unknown slice environment fails",
        slice_errors(["simulator"], ["watch/arm64"]),
        ["apple_app: unknown environment 'watch' in slice 'watch/arm64': want device, simulator"],
    ))
    checks.append(expect_equal(
        "device slice outside the simulator framework fails",
        slice_errors(["simulator"], ["simulator/arm64", "device/arm64"]),
        ["apple_app: slice 'device/arm64' is outside the simulator framework"],
    ))
    checks.append(expect_equal(
        "device x86_64 slice names the simulator",
        slice_errors(["device", "simulator"], ["device/x86_64"]),
        ["apple_ios: device builds need arch 'arm64'; 'x86_64' runs on the simulator"],
    ))
    checks.append(expect_equal(
        "missing operation fails",
        operation_errors("", "", [], False),
        ["apple_app: select one operation: want build, export, launch, publish, sign"],
    ))
    checks.append(expect_equal(
        "unknown operation fails",
        operation_errors("run", "", [], False),
        ["apple_app: unknown operation 'run': want build, export, launch, publish, sign"],
    ))
    checks.append(expect_equal("build operation is clean", operation_errors("build", "", [], False), []))
    checks.append(expect_equal(
        "sign during check fails",
        operation_errors("sign", "", [], False),
        ["apple_app: sign is an effect: allow effects explicitly; check never signs, exports, or publishes"],
    ))
    checks.append(expect_equal(
        "allowed sign without a profile fails",
        operation_errors("sign", "", ["applinks"], True),
        ["apple_app: sign needs a provisioning profile before signing"],
    ))
    checks.append(expect_equal(
        "allowed export without entitlements fails",
        operation_errors("export", "share.mobileprovision", [], True),
        ["apple_app: export needs entitlements before signing"],
    ))
    checks.append(expect_equal(
        "allowed publish selection is clean",
        operation_errors("publish", "share.mobileprovision", ["applinks"], True),
        [],
    ))
    checks.append(expect_equal("build has no pending work", pending_errors("build", "simulator"), []))
    checks.append(expect_equal(
        "simulator launch stays pending",
        pending_errors("launch", "simulator"),
        ["apple_app: simulator execution stays pending: run on a qualified macOS executor with Xcode; no simulator or device launch is qualified from this host"],
    ))
    checks.append(expect_equal(
        "device launch stays pending",
        pending_errors("launch", "device"),
        ["apple_app: device execution stays pending: run on a qualified macOS executor with Xcode; the simulator does not stand in for the device"],
    ))
    checks.append(expect_equal(
        "allowed sign stays pending",
        pending_errors("sign", "simulator"),
        ["apple_app: sign stays pending: run on a qualified macOS executor with Xcode; signing and export need supplied credentials"],
    ))
    checks.append(expect_equal("clean simulator app", app_errors(_app()), []))
    checks.append(expect_equal(
        "clean device app",
        app_errors(_app(environment = "device")),
        [],
    ))
    checks.append(expect_equal(
        "explicit cell is clean",
        app_errors(_app(
            arch = "x86_64",
            triple = "x86_64-apple-ios-sim",
            sdk = "iphonesimulator",
            frameworks = ["Foundation", "UIKit"],
            resources = ["share.dat"],
            entitlements = ["applinks"],
            operation = "launch",
        )),
        [],
    ))
    checks.append(expect_match(
        "missing bundle identifier fails the app",
        app_errors(_app(bundle_id = "")),
        "apple_app: select one bundle identifier",
    ))
    checks.append(expect_match(
        "device triple on the simulator fails the app",
        app_errors(_app(triple = "aarch64-apple-ios")),
        "apple_ios: triple 'aarch64-apple-ios' disagrees with simulator/arm64",
    ))
    checks.append(expect_match(
        "escaping resource fails the app",
        app_errors(_app(resources = ["../share.dat"])),
        "apple_app: resource '../share.dat' escapes the bundle",
    ))
    checks.append(expect_match(
        "sign during check fails the app",
        app_errors(_app(operation = "sign")),
        "apple_app: sign is an effect",
    ))
    checks.append(expect_equal("clean framework", xcframework_errors("share", ["simulator"], ["simulator/arm64"]), []))
    checks.append(expect_equal(
        "clean dual-environment framework",
        xcframework_errors("share", ["device", "simulator"], ["device/arm64", "simulator/arm64", "simulator/x86_64"]),
        [],
    ))
    checks.append(expect_equal(
        "missing framework name fails",
        xcframework_errors("", ["simulator"], ["simulator/arm64"]),
        ["apple_app: select one framework name"],
    ))
    checks.append(expect_equal(
        "missing slices fail",
        xcframework_errors("share", ["simulator"], []),
        ["apple_app: declare one slice: want 'environment/arch'"],
    ))
    checks.append(expect_equal(
        "unknown served environment fails",
        xcframework_errors("share", ["watch"], ["simulator/arm64"]),
        ["apple_app: unknown environment 'watch': want device, simulator", "apple_app: slice 'simulator/arm64' is outside the watch framework"],
    ))
    checks.append(expect_true("build sorts before launch", operations()[0] == "build"))
    checks.append(expect_true("default framework serves nothing", len(_framework(served = []).served) == 0))
    return checks

def apple_app_tests(name):
    """Declares the composed app bundle and framework pin test."""
    starlark_test(
        name = name,
        mode = "load",
        checks = _app_checks(),
    )
