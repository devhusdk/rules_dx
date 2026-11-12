"""Apple iOS and provisioning pin and selection coverage."""

load("//apple/private/ios:defs.bzl", "CELL_SETTINGS", "DEVICE_ENV", "SIMULATOR_ENV", "cell_settings", "deployment_env", "framework_linkopts", "ios_platform_errors", "platform_for", "platform_slice", "split_platform")
load("//apple/private/ios:validate.bzl", "ios_arch_errors", "ios_arches", "ios_config_errors", "ios_default_config", "ios_deployment_errors", "ios_environment_errors", "ios_environments", "ios_framework_errors", "ios_sdk_errors", "ios_sdk_for", "ios_triple_errors", "ios_triple_for", "ios_xcode_errors")
load("//apple/private/provisioning:defs.bzl", "LOCAL_ROUTE", "MANAGED_ROUTE", "MINIMUM_MACOS", "MINIMUM_XCODE", "QUALIFIED_HOST", "REQUIRED_LICENSES", "XCODE_LINEAGE", "ceiling_for", "deployment_floor", "host_constraints")
load("//apple/private/provisioning:validate.bzl", "provisioning_default_selection", "provisioning_errors", "provisioning_host_errors", "provisioning_hosts", "provisioning_license_errors", "provisioning_route_errors", "provisioning_routes", "provisioning_sdk_errors", "provisioning_sdk_names", "provisioning_version_errors", "provisioning_xcode_errors")
load("//core:defs.bzl", "expect_contains", "expect_equal", "expect_false", "expect_true", "starlark_test")

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

def _ios_selection(**kwargs):
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
    checks.append(expect_equal("device alias names its slice", platform_slice(Label("//apple:ios_arm64")), "device/arm64"))
    checks.append(expect_equal("simulator alias names its slice", platform_slice(Label("//apple:ios_sim_arm64")), "simulator/arm64"))
    checks.append(expect_equal("x86_64 alias names its slice", platform_slice(Label("//apple:ios_x86_64")), "simulator/x86_64"))
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
    checks.append(expect_equal("clean simulator selection", ios_config_errors(_ios_selection()), []))
    checks.append(expect_equal(
        "clean device selection",
        ios_config_errors(_ios_selection(environment = "device")),
        [],
    ))
    checks.append(expect_equal(
        "explicit cell is clean",
        ios_config_errors(_ios_selection(
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
        ios_config_errors(_ios_selection(environment = "", arch = "")),
        "apple_ios: select one environment: want device, simulator",
    ))
    checks.append(expect_contains(
        "device x86_64 fails the selection",
        ios_config_errors(_ios_selection(environment = "device", arch = "x86_64")),
        "apple_ios: device builds need arch 'arm64'; 'x86_64' runs on the simulator",
    ))
    checks.append(expect_contains(
        "device triple on the simulator fails the selection",
        ios_config_errors(_ios_selection(triple = "aarch64-apple-ios")),
        "apple_ios: triple 'aarch64-apple-ios' disagrees with simulator/arm64: want 'aarch64-apple-ios-sim'",
    ))
    checks.append(expect_contains(
        "too-new deployment fails the selection",
        ios_config_errors(_ios_selection(deployment = "19.0")),
        "apple_ios: deployment_target '19.0' is above the Xcode 16 'iphonesimulator' ceiling 18.0",
    ))
    checks.append(expect_true("device and simulator stay distinct", DEVICE_ENV != SIMULATOR_ENV))
    checks.append(expect_false("device and simulator share no triple", ios_triple_for("device", "arm64") == ios_triple_for("simulator", "arm64")))
    return checks

def apple_ios_tests(name, size):
    """Declares the Rust iOS library inventory pin test."""
    starlark_test(
        name = name,
        size = size,
        mode = "load",
        checks = _ios_checks(),
    )

def _provisioning_selection(**kwargs):
    base = provisioning_default_selection(kwargs.get("route", "local"))
    fields = [
        "accept_licenses",
        "allow_download",
        "api_token",
        "credential_file",
        "deployment",
        "endpoint",
        "host",
        "macos_version",
        "route",
        "sdk",
        "xcode_version",
    ]
    values = {key: kwargs.get(key, getattr(base, key)) for key in fields}
    if "route" not in kwargs:
        values["route"] = "local"
    if "host" not in kwargs:
        values["host"] = QUALIFIED_HOST
    if "sdk" not in kwargs:
        values["sdk"] = "iphoneos"
    if "xcode_version" not in kwargs and values["route"] == "local":
        values["xcode_version"] = "16.2"
    return struct(**values)

def _apple_checks():
    checks = []
    checks.append(expect_equal("routes", provisioning_routes(), ["local", "managed"]))
    checks.append(expect_equal("hosts", provisioning_hosts(), ["macos_arm64"]))
    checks.append(expect_equal("minimum xcode", MINIMUM_XCODE, "16.0"))
    checks.append(expect_equal("xcode lineage", XCODE_LINEAGE, "16."))
    checks.append(expect_equal("minimum macos", MINIMUM_MACOS, "14.5"))
    checks.append(expect_equal("qualified host", QUALIFIED_HOST, "macos_arm64"))
    checks.append(expect_equal("required licenses", REQUIRED_LICENSES, ["apple-xcode-license"]))
    checks.append(expect_equal("sdk names", provisioning_sdk_names(), ["iphoneos", "iphonesimulator", "macosx"]))
    checks.append(expect_equal("iphoneos floor", deployment_floor("iphoneos"), "17.0"))
    checks.append(expect_equal("simulator floor", deployment_floor("iphonesimulator"), "17.0"))
    checks.append(expect_equal("macos sdk floor", deployment_floor("macosx"), "14.0"))
    checks.append(expect_equal("unknown sdk floor is empty", deployment_floor("watchos"), ""))
    checks.append(expect_equal("iphoneos ceiling", ceiling_for("iphoneos"), "18.0"))
    checks.append(expect_equal("simulator ceiling", ceiling_for("iphonesimulator"), "18.0"))
    checks.append(expect_equal("macos sdk ceiling", ceiling_for("macosx"), "15.0"))
    checks.append(expect_equal("unknown sdk ceiling is empty", ceiling_for("watchos"), ""))
    checks.append(expect_equal(
        "host constraints",
        host_constraints("macos_arm64"),
        ["@platforms//os:macos", "@platforms//cpu:arm64"],
    ))
    checks.append(expect_equal("unknown host constraints are empty", host_constraints("linux_x86_64"), []))
    checks.append(expect_equal(
        "missing route fails",
        provisioning_route_errors(""),
        ["apple: select one execution route: want local, managed"],
    ))
    checks.append(expect_equal("local route is clean", provisioning_route_errors("local"), []))
    checks.append(expect_equal("managed route is clean", provisioning_route_errors("managed"), []))
    checks.append(expect_equal(
        "unknown route fails",
        provisioning_route_errors("remote"),
        ["apple: unknown route 'remote': want local, managed"],
    ))
    checks.append(expect_equal("qualified host is clean", provisioning_host_errors("macos_arm64"), []))
    checks.append(expect_equal(
        "unknown host fails",
        provisioning_host_errors("linux_x86_64"),
        ["apple: unknown host 'linux_x86_64': want macos_arm64"],
    ))
    checks.append(expect_equal("empty xcode is clean", provisioning_xcode_errors(""), []))
    checks.append(expect_equal("lineage xcode is clean", provisioning_xcode_errors("16.2"), []))
    checks.append(expect_equal(
        "old xcode fails",
        provisioning_xcode_errors("15.4"),
        ["apple: xcode '15.4' is outside 16"],
    ))
    checks.append(expect_equal(
        "freeform xcode fails",
        provisioning_xcode_errors("sixteen"),
        ["apple: xcode_version 'sixteen' is not a dotted version"],
    ))
    checks.append(expect_equal("empty version is clean", provisioning_version_errors("macos_version", "", "14.5"), []))
    checks.append(expect_equal("floor version is clean", provisioning_version_errors("macos_version", "14.5", "14.5"), []))
    checks.append(expect_equal("newer version is clean", provisioning_version_errors("macos_version", "15.1", "14.5"), []))
    checks.append(expect_equal(
        "downgraded version fails",
        provisioning_version_errors("macos_version", "13.6", "14.5"),
        ["apple: macos_version '13.6' is below 14.5"],
    ))
    checks.append(expect_equal(
        "freeform version fails",
        provisioning_version_errors("macos_version", "sonoma", "14.5"),
        ["apple: macos_version 'sonoma' is not a dotted version"],
    ))
    checks.append(expect_equal(
        "missing sdk fails",
        provisioning_sdk_errors("", ""),
        ["apple: select one SDK: want iphoneos, iphonesimulator, macosx"],
    ))
    checks.append(expect_equal("implied floor is clean", provisioning_sdk_errors("iphoneos", ""), []))
    checks.append(expect_equal("floor deployment is clean", provisioning_sdk_errors("iphoneos", "17.0"), []))
    checks.append(expect_equal("newer deployment is clean", provisioning_sdk_errors("macosx", "15.0"), []))
    checks.append(expect_equal(
        "unqualified sdk fails",
        provisioning_sdk_errors("watchos", ""),
        ["apple: sdk 'watchos' is outside iphoneos, iphonesimulator, macosx"],
    ))
    checks.append(expect_equal(
        "downgraded deployment fails",
        provisioning_sdk_errors("iphoneos", "16.0"),
        ["apple: deployment_target '16.0' is below 17.0"],
    ))
    checks.append(expect_equal("accepted license is clean", provisioning_license_errors(["apple-xcode-license"]), []))
    checks.append(expect_equal(
        "missing license fails",
        provisioning_license_errors([]),
        ["apple: accept apple-xcode-license before provisioning; add them to the accept_licenses attribute"],
    ))
    checks.append(expect_equal(
        "missing license names the target",
        provisioning_license_errors([], "local_simulator"),
        ["apple: accept apple-xcode-license before provisioning; add them to the accept_licenses attribute of local_simulator"],
    ))
    checks.append(expect_equal(
        "selection passes its target to the license error",
        provisioning_errors(_provisioning_selection(accept_licenses = []), "missing_license"),
        ["apple: accept apple-xcode-license before provisioning; add them to the accept_licenses attribute of missing_license"],
    ))
    checks.append(expect_equal("clean local selection", provisioning_errors(_provisioning_selection()), []))
    checks.append(expect_equal(
        "clean managed selection",
        provisioning_errors(_provisioning_selection(
            route = "managed",
            xcode_version = "",
            endpoint = "https://mac.example.com",
            credential_file = "/run/secrets/mac",
        )),
        [],
    ))
    checks.append(expect_equal(
        "pinned managed xcode is clean",
        provisioning_errors(_provisioning_selection(
            route = "managed",
            xcode_version = "16.2",
            endpoint = "https://mac.example.com",
            credential_file = "/run/secrets/mac",
        )),
        [],
    ))
    checks.append(expect_contains(
        "missing route fails the selection",
        provisioning_errors(_provisioning_selection(route = "", host = "", sdk = "")),
        "apple: select one execution route: want local, managed",
    ))
    checks.append(expect_contains(
        "linux host cannot run local xcode",
        provisioning_errors(_provisioning_selection(host = "linux_x86_64")),
        "apple: local route needs a macos_arm64 executor; Xcode and the simulator do not run on 'linux_x86_64'",
    ))
    checks.append(expect_contains(
        "missing local xcode fails",
        provisioning_errors(_provisioning_selection(xcode_version = "")),
        "apple: local route needs the installed Xcode version in xcode_version",
    ))
    checks.append(expect_contains(
        "managed route needs an endpoint",
        provisioning_errors(_provisioning_selection(route = "managed", xcode_version = "", endpoint = "")),
        "apple: managed route needs an endpoint naming the macOS executor",
    ))
    checks.append(expect_contains(
        "managed route needs a credential file",
        provisioning_errors(_provisioning_selection(route = "managed", xcode_version = "")),
        "apple: managed route needs credential_file; credentials never travel inline",
    ))
    checks.append(expect_contains(
        "inline secrets fail",
        provisioning_errors(_provisioning_selection(api_token = "s3cret")),
        "apple: api_token must stay empty; pass credential_file instead",
    ))
    checks.append(expect_contains(
        "unattended downloads fail",
        provisioning_errors(_provisioning_selection(allow_download = True)),
        "apple: Xcode has no unattended download; use the local route or a managed endpoint",
    ))
    checks.append(expect_true("local and managed stay distinct", LOCAL_ROUTE != MANAGED_ROUTE))
    checks.append(expect_false("sdk floors leave no gap", deployment_floor("iphoneos") == ""))
    return checks

def apple_provisioning_tests(name, size):
    """Declares the Apple provisioning inventory pin test."""
    starlark_test(
        name = name,
        size = size,
        mode = "load",
        checks = _apple_checks(),
    )
