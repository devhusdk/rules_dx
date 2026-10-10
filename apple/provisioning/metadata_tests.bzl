"""Apple provisioning pin and selection coverage."""

load("//libs/starlark:defs.bzl", "expect_contains", "expect_equal", "expect_false", "expect_true", "starlark_test")
load(
    ":defs.bzl",
    "LOCAL_ROUTE",
    "MANAGED_ROUTE",
    "MINIMUM_MACOS",
    "MINIMUM_XCODE",
    "QUALIFIED_HOST",
    "REQUIRED_LICENSES",
    "XCODE_LINEAGE",
    "default_selection",
    "deployment_floor",
    "host_constraints",
    "host_errors",
    "hosts",
    "license_errors",
    "provisioning_errors",
    "route_errors",
    "routes",
    "sdk_errors",
    "sdk_names",
    "version_errors",
    "xcode_errors",
)

def _selection(**kwargs):
    base = default_selection(kwargs.get("route", "local"))
    fields = [
        "accept_licenses",
        "allow_download",
        "api_token",
        "credential_file",
        "deployment",
        "endpoint",
        "host",
        "local_path",
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
    checks.append(expect_equal("routes", routes(), ["local", "managed"]))
    checks.append(expect_equal("hosts", hosts(), ["macos_arm64"]))
    checks.append(expect_equal("minimum xcode", MINIMUM_XCODE, "16.0"))
    checks.append(expect_equal("xcode lineage", XCODE_LINEAGE, "16."))
    checks.append(expect_equal("minimum macos", MINIMUM_MACOS, "14.5"))
    checks.append(expect_equal("qualified host", QUALIFIED_HOST, "macos_arm64"))
    checks.append(expect_equal("required licenses", REQUIRED_LICENSES, ["apple-xcode-license"]))
    checks.append(expect_equal("sdk names", sdk_names(), ["iphoneos", "iphonesimulator", "macosx"]))
    checks.append(expect_equal("iphoneos floor", deployment_floor("iphoneos"), "17.0"))
    checks.append(expect_equal("simulator floor", deployment_floor("iphonesimulator"), "17.0"))
    checks.append(expect_equal("macos sdk floor", deployment_floor("macosx"), "14.0"))
    checks.append(expect_equal("unknown sdk floor is empty", deployment_floor("watchos"), ""))
    checks.append(expect_equal(
        "host constraints",
        host_constraints("macos_arm64"),
        ["@platforms//os:macos", "@platforms//cpu:arm64"],
    ))
    checks.append(expect_equal("unknown host constraints are empty", host_constraints("linux_x86_64"), []))
    checks.append(expect_equal(
        "missing route fails",
        route_errors(""),
        ["apple: select one execution route: want local, managed"],
    ))
    checks.append(expect_equal("local route is clean", route_errors("local"), []))
    checks.append(expect_equal("managed route is clean", route_errors("managed"), []))
    checks.append(expect_equal(
        "unknown route fails",
        route_errors("remote"),
        ["apple: unknown route 'remote': want local, managed"],
    ))
    checks.append(expect_equal("qualified host is clean", host_errors("macos_arm64"), []))
    checks.append(expect_equal(
        "unknown host fails",
        host_errors("linux_x86_64"),
        ["apple: unknown host 'linux_x86_64': want macos_arm64"],
    ))
    checks.append(expect_equal("empty xcode is clean", xcode_errors(""), []))
    checks.append(expect_equal("lineage xcode is clean", xcode_errors("16.2"), []))
    checks.append(expect_equal(
        "old xcode fails",
        xcode_errors("15.4"),
        ["apple: xcode '15.4' is outside 16"],
    ))
    checks.append(expect_equal(
        "freeform xcode fails",
        xcode_errors("sixteen"),
        ["apple: xcode_version 'sixteen' is not a dotted version"],
    ))
    checks.append(expect_equal("empty version is clean", version_errors("macos_version", "", "14.5"), []))
    checks.append(expect_equal("floor version is clean", version_errors("macos_version", "14.5", "14.5"), []))
    checks.append(expect_equal("newer version is clean", version_errors("macos_version", "15.1", "14.5"), []))
    checks.append(expect_equal(
        "downgraded version fails",
        version_errors("macos_version", "13.6", "14.5"),
        ["apple: macos_version '13.6' is below 14.5"],
    ))
    checks.append(expect_equal(
        "freeform version fails",
        version_errors("macos_version", "sonoma", "14.5"),
        ["apple: macos_version 'sonoma' is not a dotted version"],
    ))
    checks.append(expect_equal(
        "missing sdk fails",
        sdk_errors("", ""),
        ["apple: select one SDK: want iphoneos, iphonesimulator, macosx"],
    ))
    checks.append(expect_equal("implied floor is clean", sdk_errors("iphoneos", ""), []))
    checks.append(expect_equal("floor deployment is clean", sdk_errors("iphoneos", "17.0"), []))
    checks.append(expect_equal("newer deployment is clean", sdk_errors("macosx", "15.0"), []))
    checks.append(expect_equal(
        "unqualified sdk fails",
        sdk_errors("watchos", ""),
        ["apple: sdk 'watchos' is outside iphoneos, iphonesimulator, macosx"],
    ))
    checks.append(expect_equal(
        "downgraded deployment fails",
        sdk_errors("iphoneos", "16.0"),
        ["apple: deployment_target '16.0' is below 17.0"],
    ))
    checks.append(expect_equal("accepted license is clean", license_errors(["apple-xcode-license"]), []))
    checks.append(expect_equal(
        "missing license fails",
        license_errors([]),
        ["apple: accept apple-xcode-license before provisioning"],
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
