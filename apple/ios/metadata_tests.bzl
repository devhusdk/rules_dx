"""Rust iOS library pin and selection coverage."""

load("//libs/starlark:defs.bzl", "expect_contains", "expect_equal", "expect_false", "expect_true", "starlark_test")
load(
    ":defs.bzl",
    "DEVICE_ENV",
    "SIMULATOR_ENV",
    "arch_errors",
    "arches",
    "ceiling_for",
    "config_errors",
    "default_config",
    "deployment_errors",
    "environment_errors",
    "environments",
    "execution_errors",
    "framework_errors",
    "sdk_errors",
    "sdk_for",
    "triple_errors",
    "triple_for",
    "xcode_errors",
)

def _selection(**kwargs):
    base = default_config(kwargs.get("environment", "simulator"))
    fields = [
        "arch",
        "crate_features",
        "deployment",
        "environment",
        "frameworks",
        "rustc_flags",
        "sdk",
        "triple",
        "xcode_version",
    ]
    values = {key: kwargs.get(key, getattr(base, key)) for key in fields}
    if "environment" not in kwargs:
        values["environment"] = "simulator"
    if "arch" not in kwargs:
        values["arch"] = "arm64"
    if "xcode_version" not in kwargs:
        values["xcode_version"] = "16.2"
    return struct(**values)

def _ios_checks():
    checks = []
    checks.append(expect_equal("environments", environments(), ["device", "simulator"]))
    checks.append(expect_equal("device arches", arches("device"), ["arm64"]))
    checks.append(expect_equal("simulator arches", arches("simulator"), ["arm64", "x86_64"]))
    checks.append(expect_equal("unknown environment arches are empty", arches("watch"), []))
    checks.append(expect_equal("device sdk", sdk_for("device"), "iphoneos"))
    checks.append(expect_equal("simulator sdk", sdk_for("simulator"), "iphonesimulator"))
    checks.append(expect_equal("unknown environment sdk is empty", sdk_for("watch"), ""))
    checks.append(expect_equal("device triple", triple_for("device", "arm64"), "aarch64-apple-ios"))
    checks.append(expect_equal("simulator arm64 triple", triple_for("simulator", "arm64"), "aarch64-apple-ios-sim"))
    checks.append(expect_equal("simulator x86_64 triple", triple_for("simulator", "x86_64"), "x86_64-apple-ios-sim"))
    checks.append(expect_equal("device x86_64 triple is empty", triple_for("device", "x86_64"), ""))
    checks.append(expect_equal("unknown environment triple is empty", triple_for("watch", "arm64"), ""))
    checks.append(expect_equal("iphoneos ceiling", ceiling_for("iphoneos"), "18.0"))
    checks.append(expect_equal("simulator ceiling", ceiling_for("iphonesimulator"), "18.0"))
    checks.append(expect_equal("unknown sdk ceiling is empty", ceiling_for("macosx"), ""))
    checks.append(expect_equal(
        "missing environment fails",
        environment_errors(""),
        ["apple_ios: select one environment: want device, simulator"],
    ))
    checks.append(expect_equal("device environment is clean", environment_errors("device"), []))
    checks.append(expect_equal("simulator environment is clean", environment_errors("simulator"), []))
    checks.append(expect_equal(
        "unknown environment fails",
        environment_errors("watch"),
        ["apple_ios: unknown environment 'watch': want device, simulator"],
    ))
    checks.append(expect_equal(
        "missing arch fails",
        arch_errors("simulator", ""),
        ["apple_ios: select one arch for 'simulator': want arm64, x86_64"],
    ))
    checks.append(expect_equal("device arm64 is clean", arch_errors("device", "arm64"), []))
    checks.append(expect_equal("simulator x86_64 is clean", arch_errors("simulator", "x86_64"), []))
    checks.append(expect_equal(
        "device x86_64 names the simulator",
        arch_errors("device", "x86_64"),
        ["apple_ios: device builds need arch 'arm64'; 'x86_64' runs on the simulator"],
    ))
    checks.append(expect_equal(
        "simulator armv7 fails",
        arch_errors("simulator", "armv7"),
        ["apple_ios: arch 'armv7' is outside simulator: want arm64, x86_64"],
    ))
    checks.append(expect_equal("unknown environment arch is clean", arch_errors("watch", "arm64"), []))
    checks.append(expect_equal("empty triple is clean", triple_errors("simulator", "arm64", ""), []))
    checks.append(expect_equal(
        "matching triple is clean",
        triple_errors("simulator", "arm64", "aarch64-apple-ios-sim"),
        [],
    ))
    checks.append(expect_equal(
        "device triple on the simulator fails",
        triple_errors("simulator", "arm64", "aarch64-apple-ios"),
        ["apple_ios: triple 'aarch64-apple-ios' disagrees with simulator/arm64: want 'aarch64-apple-ios-sim'"],
    ))
    checks.append(expect_equal(
        "simulator triple on the device fails",
        triple_errors("device", "arm64", "aarch64-apple-ios-sim"),
        ["apple_ios: triple 'aarch64-apple-ios-sim' disagrees with device/arm64: want 'aarch64-apple-ios'"],
    ))
    checks.append(expect_equal("empty sdk is clean", sdk_errors("device", ""), []))
    checks.append(expect_equal("matching sdk is clean", sdk_errors("device", "iphoneos"), []))
    checks.append(expect_equal(
        "simulator sdk on the device fails",
        sdk_errors("device", "iphonesimulator"),
        ["apple_ios: sdk 'iphonesimulator' disagrees with 'device': want 'iphoneos'"],
    ))
    checks.append(expect_equal("empty deployment is clean", deployment_errors("device", ""), []))
    checks.append(expect_equal("floor deployment is clean", deployment_errors("device", "17.0"), []))
    checks.append(expect_equal("ceiling deployment is clean", deployment_errors("device", "18.0"), []))
    checks.append(expect_equal(
        "downgraded deployment fails",
        deployment_errors("device", "16.0"),
        ["apple_ios: deployment_target '16.0' is below 17.0"],
    ))
    checks.append(expect_equal(
        "freeform deployment fails",
        deployment_errors("device", "eighteen"),
        ["apple_ios: deployment_target 'eighteen' is not a dotted version"],
    ))
    checks.append(expect_equal(
        "too-new deployment fails",
        deployment_errors("device", "19.0"),
        ["apple_ios: deployment_target '19.0' is above the Xcode 16 'iphoneos' ceiling 18.0"],
    ))
    checks.append(expect_equal("empty xcode is clean", xcode_errors(""), []))
    checks.append(expect_equal("lineage xcode is clean", xcode_errors("16.2"), []))
    checks.append(expect_equal(
        "old xcode fails",
        xcode_errors("15.4"),
        ["apple_ios: xcode '15.4' is outside 16"],
    ))
    checks.append(expect_equal(
        "freeform xcode fails",
        xcode_errors("sixteen"),
        ["apple_ios: xcode_version 'sixteen' is not a dotted version"],
    ))
    checks.append(expect_equal("no frameworks are clean", framework_errors([]), []))
    checks.append(expect_equal("named frameworks are clean", framework_errors(["Foundation", "UIKit"]), []))
    checks.append(expect_equal(
        "empty framework fails",
        framework_errors(["Foundation", ""]),
        ["apple_ios: framework names must not be empty"],
    ))
    checks.append(expect_equal(
        "simulator execution stays pending",
        execution_errors("simulator"),
        ["apple_ios: simulator execution stays pending: run on a qualified macOS executor with Xcode; no simulator or device launch is qualified from this host"],
    ))
    checks.append(expect_equal("clean simulator selection", config_errors(_selection()), []))
    checks.append(expect_equal(
        "clean device selection",
        config_errors(_selection(environment = "device")),
        [],
    ))
    checks.append(expect_equal(
        "explicit cell is clean",
        config_errors(_selection(
            environment = "simulator",
            arch = "x86_64",
            triple = "x86_64-apple-ios-sim",
            sdk = "iphonesimulator",
            deployment = "17.0",
            frameworks = ["Foundation"],
            crate_features = ["share"],
            rustc_flags = ["--cfg", "dx_ios"],
        )),
        [],
    ))
    checks.append(expect_contains(
        "missing environment fails the selection",
        config_errors(_selection(environment = "", arch = "")),
        "apple_ios: select one environment: want device, simulator",
    ))
    checks.append(expect_contains(
        "device x86_64 fails the selection",
        config_errors(_selection(environment = "device", arch = "x86_64")),
        "apple_ios: device builds need arch 'arm64'; 'x86_64' runs on the simulator",
    ))
    checks.append(expect_contains(
        "device triple on the simulator fails the selection",
        config_errors(_selection(triple = "aarch64-apple-ios")),
        "apple_ios: triple 'aarch64-apple-ios' disagrees with simulator/arm64: want 'aarch64-apple-ios-sim'",
    ))
    checks.append(expect_contains(
        "too-new deployment fails the selection",
        config_errors(_selection(deployment = "19.0")),
        "apple_ios: deployment_target '19.0' is above the Xcode 16 'iphonesimulator' ceiling 18.0",
    ))
    checks.append(expect_true("device and simulator stay distinct", DEVICE_ENV != SIMULATOR_ENV))
    checks.append(expect_false("device and simulator share no triple", triple_for("device", "arm64") == triple_for("simulator", "arm64")))
    return checks

def apple_ios_tests(name):
    """Declares the Rust iOS library inventory pin test."""
    starlark_test(
        name = name,
        mode = "load",
        checks = _ios_checks(),
    )
