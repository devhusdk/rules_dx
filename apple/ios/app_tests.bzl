"""Simulator application and embeddable framework pin and selection coverage."""

load("//libs/starlark:defs.bzl", "expect_contains", "expect_equal", "starlark_test")
load(
    ":app.bzl",
    "app_errors",
    "app_resource_errors",
    "bundle_id_errors",
    "bundle_version_errors",
    "default_app",
    "default_framework",
    "entitlement_errors",
    "framework_errors",
    "framework_resource_errors",
    "launch_errors",
    "signing_errors",
    "slice_errors",
)

def _app(**kwargs):
    base = default_app(kwargs.get("environment", "simulator"))
    fields = [
        "arch",
        "bundle_id",
        "deployment",
        "entitlements",
        "environment",
        "export_ipa",
        "library",
        "provisioning_profile",
        "resources",
        "signing_identity",
        "version",
        "xcode_version",
    ]
    values = {key: kwargs.get(key, getattr(base, key)) for key in fields}
    if "bundle_id" not in kwargs:
        values["bundle_id"] = "com.example.share"
    if "version" not in kwargs:
        values["version"] = "1.0"
    if "library" not in kwargs:
        values["library"] = "share"
    if "xcode_version" not in kwargs:
        values["xcode_version"] = "16.2"
    return struct(**values)

def _framework(**kwargs):
    base = default_framework()
    fields = ["bundle_id", "resources", "slices", "version"]
    values = {key: kwargs.get(key, getattr(base, key)) for key in fields}
    if "bundle_id" not in kwargs:
        values["bundle_id"] = "com.example.share"
    if "version" not in kwargs:
        values["version"] = "1.0"
    if "slices" not in kwargs:
        values["slices"] = ["device/arm64", "simulator/arm64"]
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
    checks.append(expect_equal("unsigned simulator is clean", signing_errors("simulator", "", ""), []))
    checks.append(expect_equal(
        "signed simulator fails",
        signing_errors("simulator", "share.mobileprovision", "Apple Development"),
        ["apple_app: simulator runs unsigned; leave provisioning_profile and signing_identity empty"],
    ))
    checks.append(expect_equal(
        "unsigned device fails twice",
        signing_errors("device", "", ""),
        [
            "apple_app: device distribution needs provisioning_profile; credentials travel by file only",
            "apple_app: device distribution needs signing_identity",
        ],
    ))
    checks.append(expect_equal(
        "signed device is clean",
        signing_errors("device", "share.mobileprovision", "Apple Development"),
        [],
    ))
    checks.append(expect_equal(
        "simulator launch stays pending",
        launch_errors("simulator"),
        ["apple_app: simulator launch stays pending: run on a qualified macOS executor with Xcode; no simulator or device launch is qualified from this host"],
    ))
    checks.append(expect_equal("clean simulator app", app_errors(_app()), []))
    checks.append(expect_equal(
        "clean signed device app",
        app_errors(_app(
            environment = "device",
            provisioning_profile = "share.mobileprovision",
            signing_identity = "Apple Development",
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
        "apple_app: app composes one qualified Rust library cell in library",
    ))
    checks.append(expect_contains(
        "unsigned device fails the app",
        app_errors(_app(environment = "device")),
        "apple_app: device distribution needs provisioning_profile; credentials travel by file only",
    ))
    checks.append(expect_contains(
        "export during check fails the app",
        app_errors(_app(export_ipa = True)),
        "apple_app: export is a distinct explicitly applied operation; Check never exports an IPA",
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
    checks.append(expect_contains(
        "wrong framework slice fails",
        framework_errors(_framework(slices = ["device/arm64", "simulator/armv7"])),
        "apple_framework: arch 'armv7' is outside simulator in slice 'simulator/armv7': want arm64, x86_64",
    ))
    return checks

def apple_ios_app_tests(name):
    """Declares the simulator app and framework inventory pin test."""
    starlark_test(
        name = name,
        mode = "load",
        checks = _app_checks(),
    )
