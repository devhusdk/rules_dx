"""Rust iOS library pin and selection coverage."""

load("@rules_dx//core:defs.bzl", "expect_contains", "expect_equal", "expect_false", "expect_true", "starlark_test")
load(":defs.bzl", "CELL_SETTINGS", "DEVICE_ENV", "SIMULATOR_ENV", "cell_settings", "deployment_env", "framework_linkopts", "ios_platform_errors", "ios_platforms", "platform_for", "platform_slice", "split_platform")
load(":validate.bzl", "ios_arch_errors", "ios_arches", "ios_config_errors", "ios_default_config", "ios_deployment_errors", "ios_environment_errors", "ios_environments", "ios_framework_errors", "ios_sdk_errors", "ios_sdk_for", "ios_triple_errors", "ios_triple_for", "ios_xcode_errors")

_IOS_DEVICE = Label("@apple_support//platforms:ios_arm64")

_IOS_SIM = Label("@apple_support//platforms:ios_sim_arm64")

_IOS_X86 = Label("@apple_support//platforms:ios_x86_64")

_MACOS = Label("@apple_support//platforms:macos_arm64")

_LINUX = Label("@platforms//host")

_ORDER = [
    "//command_line_option:platforms",
    "//command_line_option:apple_platform_type",
    "//command_line_option:ios_minimum_os",
    "//command_line_option:xcode_version",
]

def _settings(platform):
    return {
        "//command_line_option:apple_platform_type": "macos",
        "//command_line_option:ios_minimum_os": None,
        "//command_line_option:platforms": [platform],
        "//command_line_option:xcode_version": None,
    }

def _plain(settings):
    plain = dict(settings)
    plain["//command_line_option:platforms"] = [str(platform) for platform in settings["//command_line_option:platforms"]]
    return plain

def _selection(**kwargs):
    base = ios_default_config(kwargs.get("environment", "simulator"))
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
    checks.append(expect_equal("environments", ios_environments(), ["device", "simulator"]))
    checks.append(expect_equal("device arches", ios_arches("device"), ["arm64"]))
    checks.append(expect_equal("simulator arches", ios_arches("simulator"), ["arm64", "x86_64"]))
    checks.append(expect_equal("unknown environment arches are empty", ios_arches("watch"), []))
    checks.append(expect_equal("device sdk", ios_sdk_for("device"), "iphoneos"))
    checks.append(expect_equal("simulator sdk", ios_sdk_for("simulator"), "iphonesimulator"))
    checks.append(expect_equal("unknown environment sdk is empty", ios_sdk_for("watch"), ""))
    checks.append(expect_equal("device triple", ios_triple_for("device", "arm64"), "aarch64-apple-ios"))
    checks.append(expect_equal("simulator arm64 triple", ios_triple_for("simulator", "arm64"), "aarch64-apple-ios-sim"))
    checks.append(expect_equal("simulator x86_64 triple", ios_triple_for("simulator", "x86_64"), "x86_64-apple-ios"))
    checks.append(expect_equal("device x86_64 triple is empty", ios_triple_for("device", "x86_64"), ""))
    checks.append(expect_equal("unknown environment triple is empty", ios_triple_for("watch", "arm64"), ""))
    checks.append(expect_equal("device platform splits", split_platform("device/arm64"), ("device", "arm64")))
    checks.append(expect_equal("simulator x86_64 platform splits", split_platform("simulator/x86_64"), ("simulator", "x86_64")))
    checks.append(expect_equal("empty platform splits to nothing", split_platform(""), ("", "")))
    checks.append(expect_equal("unknown platform splits to nothing", split_platform("watch/arm64"), ("", "")))
    checks.append(expect_equal("device label names its slice", platform_slice(_IOS_DEVICE), "device/arm64"))
    checks.append(expect_equal("simulator label names its slice", platform_slice(_IOS_SIM), "simulator/arm64"))
    checks.append(expect_equal("macos label has no slice", platform_slice(_MACOS), ""))
    checks.append(expect_equal("supported platform is clean", ios_platform_errors("apple_ios_lib", "device"), []))
    checks.append(expect_equal(
        "unsupported platform fails",
        ios_platform_errors("apple_ios_lib", ""),
        ["apple_ios_lib: platform takes one of " + ", ".join([str(_IOS_DEVICE), str(_IOS_SIM), str(_IOS_X86)])],
    ))
    checks.append(expect_equal(
        "missing environment fails",
        ios_environment_errors(""),
        ["apple_ios: select one environment: want device, simulator"],
    ))
    checks.append(expect_equal("device environment is clean", ios_environment_errors("device"), []))
    checks.append(expect_equal("simulator environment is clean", ios_environment_errors("simulator"), []))
    checks.append(expect_equal(
        "unknown environment fails",
        ios_environment_errors("watch"),
        ["apple_ios: unknown environment 'watch': want device, simulator"],
    ))
    checks.append(expect_equal(
        "missing arch fails",
        ios_arch_errors("simulator", ""),
        ["apple_ios: select one arch for 'simulator': want arm64, x86_64"],
    ))
    checks.append(expect_equal("device arm64 is clean", ios_arch_errors("device", "arm64"), []))
    checks.append(expect_equal("simulator x86_64 is clean", ios_arch_errors("simulator", "x86_64"), []))
    checks.append(expect_equal(
        "device x86_64 names the simulator",
        ios_arch_errors("device", "x86_64"),
        ["apple_ios: device builds need arch 'arm64'; 'x86_64' runs on the simulator"],
    ))
    checks.append(expect_equal(
        "simulator armv7 fails",
        ios_arch_errors("simulator", "armv7"),
        ["apple_ios: arch 'armv7' is outside simulator: want arm64, x86_64"],
    ))
    checks.append(expect_equal("unknown environment arch is clean", ios_arch_errors("watch", "arm64"), []))
    checks.append(expect_equal("empty triple is clean", ios_triple_errors("simulator", "arm64", ""), []))
    checks.append(expect_equal(
        "matching triple is clean",
        ios_triple_errors("simulator", "arm64", "aarch64-apple-ios-sim"),
        [],
    ))
    checks.append(expect_equal(
        "device triple on the simulator fails",
        ios_triple_errors("simulator", "arm64", "aarch64-apple-ios"),
        ["apple_ios: triple 'aarch64-apple-ios' disagrees with simulator/arm64: want 'aarch64-apple-ios-sim'"],
    ))
    checks.append(expect_equal(
        "simulator triple on the device fails",
        ios_triple_errors("device", "arm64", "aarch64-apple-ios-sim"),
        ["apple_ios: triple 'aarch64-apple-ios-sim' disagrees with device/arm64: want 'aarch64-apple-ios'"],
    ))
    checks.append(expect_equal("empty sdk is clean", ios_sdk_errors("device", ""), []))
    checks.append(expect_equal("matching sdk is clean", ios_sdk_errors("device", "iphoneos"), []))
    checks.append(expect_equal(
        "simulator sdk on the device fails",
        ios_sdk_errors("device", "iphonesimulator"),
        ["apple_ios: sdk 'iphonesimulator' disagrees with 'device': want 'iphoneos'"],
    ))
    checks.append(expect_equal("empty deployment is clean", ios_deployment_errors("device", ""), []))
    checks.append(expect_equal("floor deployment is clean", ios_deployment_errors("device", "17.0"), []))
    checks.append(expect_equal("ceiling deployment is clean", ios_deployment_errors("device", "18.0"), []))
    checks.append(expect_equal(
        "downgraded deployment fails",
        ios_deployment_errors("device", "16.0"),
        ["apple_ios: deployment_target '16.0' is below 17.0"],
    ))
    checks.append(expect_equal(
        "freeform deployment fails",
        ios_deployment_errors("device", "eighteen"),
        ["apple_ios: deployment_target 'eighteen' is not a dotted version"],
    ))
    checks.append(expect_equal(
        "too-new deployment fails",
        ios_deployment_errors("device", "19.0"),
        ["apple_ios: deployment_target '19.0' is above the Xcode 16 'iphoneos' ceiling 18.0"],
    ))
    checks.append(expect_equal("empty xcode is clean", ios_xcode_errors(""), []))
    checks.append(expect_equal("lineage xcode is clean", ios_xcode_errors("16.2"), []))
    checks.append(expect_equal(
        "old xcode fails",
        ios_xcode_errors("15.4"),
        ["apple_ios: xcode '15.4' is outside 16"],
    ))
    checks.append(expect_equal(
        "freeform xcode fails",
        ios_xcode_errors("sixteen"),
        ["apple_ios: xcode_version 'sixteen' is not a dotted version"],
    ))
    checks.append(expect_equal("no frameworks are clean", ios_framework_errors([]), []))
    checks.append(expect_equal("named frameworks are clean", ios_framework_errors(["Foundation", "UIKit"]), []))
    checks.append(expect_equal(
        "empty framework fails",
        ios_framework_errors(["Foundation", ""]),
        ["apple_ios: framework names must not be empty"],
    ))
    checks.append(expect_equal("device platform", str(platform_for("device", "arm64")), str(_IOS_DEVICE)))
    checks.append(expect_equal("simulator arm64 platform", str(platform_for("simulator", "arm64")), str(_IOS_SIM)))
    checks.append(expect_equal("simulator x86_64 platform", str(platform_for("simulator", "x86_64")), str(_IOS_X86)))
    checks.append(expect_equal("device x86_64 has no platform", platform_for("device", "x86_64"), None))
    checks.append(expect_equal("unknown environment has no platform", platform_for("watch", "arm64"), None))
    for platform in [_IOS_DEVICE, _IOS_SIM, _IOS_X86]:
        checks.append(expect_equal(
            "an iOS platform keeps its settings: " + str(platform),
            _plain(cell_settings(_settings(platform), "device", "arm64", "18.0", "16.4")),
            _plain(_settings(platform)),
        ))
    checks.append(expect_equal(
        "a macOS build moves to the simulator cell",
        _plain(cell_settings(_settings(_MACOS), "simulator", "x86_64", "18.0", "16.4")),
        {
            "//command_line_option:apple_platform_type": "ios",
            "//command_line_option:ios_minimum_os": "18.0",
            "//command_line_option:platforms": [str(_IOS_X86)],
            "//command_line_option:xcode_version": "16.4",
        },
    ))
    checks.append(expect_equal(
        "a Linux build moves to the device cell at the floor",
        _plain(cell_settings(_settings(_LINUX), "device", "arm64", "", "")),
        {
            "//command_line_option:apple_platform_type": "ios",
            "//command_line_option:ios_minimum_os": "17.0",
            "//command_line_option:platforms": [str(_IOS_DEVICE)],
            "//command_line_option:xcode_version": None,
        },
    ))
    checks.append(expect_equal(
        "an invalid cell keeps its settings",
        _plain(cell_settings(_settings(_MACOS), "device", "x86_64", "", "")),
        _plain(_settings(_MACOS)),
    ))
    checks.append(expect_equal("cell settings", CELL_SETTINGS, _ORDER))
    checks.append(expect_equal("no frameworks link no flags", framework_linkopts([]), []))
    checks.append(expect_equal(
        "frameworks link single-token flags",
        framework_linkopts(["Foundation", "UIKit"]),
        ["-Wl,-framework,Foundation", "-Wl,-framework,UIKit"],
    ))
    checks.append(expect_equal(
        "declared deployment pins rustc",
        deployment_env("device", "18.0"),
        {"IPHONEOS_DEPLOYMENT_TARGET": "18.0"},
    ))
    checks.append(expect_equal(
        "empty deployment pins rustc to the floor",
        deployment_env("simulator", ""),
        {"IPHONEOS_DEPLOYMENT_TARGET": "17.0"},
    ))
    checks.append(expect_equal("clean simulator selection", ios_config_errors(_selection()), []))
    checks.append(expect_equal(
        "clean device selection",
        ios_config_errors(_selection(environment = "device")),
        [],
    ))
    checks.append(expect_equal(
        "explicit cell is clean",
        ios_config_errors(_selection(
            environment = "simulator",
            arch = "x86_64",
            triple = "x86_64-apple-ios",
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
        ios_config_errors(_selection(environment = "", arch = "")),
        "apple_ios: select one environment: want device, simulator",
    ))
    checks.append(expect_contains(
        "device x86_64 fails the selection",
        ios_config_errors(_selection(environment = "device", arch = "x86_64")),
        "apple_ios: device builds need arch 'arm64'; 'x86_64' runs on the simulator",
    ))
    checks.append(expect_contains(
        "device triple on the simulator fails the selection",
        ios_config_errors(_selection(triple = "aarch64-apple-ios")),
        "apple_ios: triple 'aarch64-apple-ios' disagrees with simulator/arm64: want 'aarch64-apple-ios-sim'",
    ))
    checks.append(expect_contains(
        "too-new deployment fails the selection",
        ios_config_errors(_selection(deployment = "19.0")),
        "apple_ios: deployment_target '19.0' is above the Xcode 16 'iphonesimulator' ceiling 18.0",
    ))
    checks.append(expect_true("device and simulator stay distinct", DEVICE_ENV != SIMULATOR_ENV))
    checks.append(expect_false("device and simulator share no triple", ios_triple_for("device", "arm64") == ios_triple_for("simulator", "arm64")))
    return checks

def apple_ios_tests(name):
    """Declares the Rust iOS library inventory pin test."""
    starlark_test(
        name = name,
        mode = "load",
        checks = _ios_checks(),
    )
