"""Apple provisioning pin and selection coverage."""

load("//core:defs.bzl", "expect_contains", "expect_equal", "expect_false", "expect_true", "starlark_test")
load(":defs.bzl", "LOCAL_ROUTE", "MANAGED_ROUTE", "MINIMUM_MACOS", "MINIMUM_XCODE", "QUALIFIED_HOST", "REQUIRED_LICENSES", "XCODE_LINEAGE", "ceiling_for", "deployment_floor", "host_constraints")
load(":validate.bzl", "provisioning_default_selection", "provisioning_errors", "provisioning_host_errors", "provisioning_hosts", "provisioning_license_errors", "provisioning_route_errors", "provisioning_routes", "provisioning_sdk_errors", "provisioning_sdk_names", "provisioning_version_errors", "provisioning_xcode_errors")

def _selection(**kwargs):
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
        provisioning_errors(_selection(accept_licenses = []), "missing_license"),
        ["apple: accept apple-xcode-license before provisioning; add them to the accept_licenses attribute of missing_license"],
    ))
    checks.append(expect_equal("clean local selection", provisioning_errors(_selection()), []))
    checks.append(expect_equal(
        "clean managed selection",
        provisioning_errors(_selection(
            route = "managed",
            xcode_version = "",
            endpoint = "https://mac.example.com",
            credential_file = "/run/secrets/mac",
        )),
        [],
    ))
    checks.append(expect_equal(
        "pinned managed xcode is clean",
        provisioning_errors(_selection(
            route = "managed",
            xcode_version = "16.2",
            endpoint = "https://mac.example.com",
            credential_file = "/run/secrets/mac",
        )),
        [],
    ))
    checks.append(expect_contains(
        "missing route fails the selection",
        provisioning_errors(_selection(route = "", host = "", sdk = "")),
        "apple: select one execution route: want local, managed",
    ))
    checks.append(expect_contains(
        "linux host cannot run local xcode",
        provisioning_errors(_selection(host = "linux_x86_64")),
        "apple: local route needs a macos_arm64 executor; Xcode and the simulator do not run on 'linux_x86_64'",
    ))
    checks.append(expect_contains(
        "missing local xcode fails",
        provisioning_errors(_selection(xcode_version = "")),
        "apple: local route needs the installed Xcode version in xcode_version",
    ))
    checks.append(expect_contains(
        "managed route needs an endpoint",
        provisioning_errors(_selection(route = "managed", xcode_version = "", endpoint = "")),
        "apple: managed route needs an endpoint naming the macOS executor",
    ))
    checks.append(expect_contains(
        "managed route needs a credential file",
        provisioning_errors(_selection(route = "managed", xcode_version = "")),
        "apple: managed route needs credential_file; credentials never travel inline",
    ))
    checks.append(expect_contains(
        "inline secrets fail",
        provisioning_errors(_selection(api_token = "s3cret")),
        "apple: api_token must stay empty; pass credential_file instead",
    ))
    checks.append(expect_contains(
        "unattended downloads fail",
        provisioning_errors(_selection(allow_download = True)),
        "apple: Xcode has no unattended download; use the local route or a managed endpoint",
    ))
    checks.append(expect_true("local and managed stay distinct", LOCAL_ROUTE != MANAGED_ROUTE))
    checks.append(expect_false("sdk floors leave no gap", deployment_floor("iphoneos") == ""))
    return checks

def apple_provisioning_tests(name):
    """Declares the Apple provisioning inventory pin test."""
    starlark_test(
        name = name,
        mode = "load",
        checks = _apple_checks(),
    )
