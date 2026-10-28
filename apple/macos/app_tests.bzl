"""macOS application selection coverage."""

load("//apple/bundle:defs.bzl", "bundle_id_errors", "bundle_version_errors")
load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load(":app.bzl", "default_macos_app", "macos_app_errors", "owner_relative_path", "plugin_errors")

def _app(**kwargs):
    base = default_macos_app()
    fields = [
        "arch",
        "bundle_id",
        "deployment",
        "entitlements",
        "frameworks",
        "library",
        "plugins",
        "provisioning_profile",
        "resources",
        "srcs",
        "version",
        "xcode_version",
    ]
    values = {key: kwargs.get(key, getattr(base, key)) for key in fields}
    defaults = {
        "bundle_id": "com.example.ledger",
        "deployment": "14.0",
        "library": "//ledger:ledger_rust",
        "srcs": ["main.m"],
        "version": "1.0",
        "xcode_version": "16.2",
    }
    for key, value in defaults.items():
        if key not in kwargs:
            values[key] = value
    return struct(**values)

def _app_checks():
    checks = []
    checks.append(expect_equal("clean app has no errors", macos_app_errors(_app()), []))
    checks.append(expect_equal("ceiling deployment is clean", macos_app_errors(_app(deployment = "15.0")), []))
    checks.append(expect_equal(
        "empty bundle id fails",
        macos_app_errors(_app(bundle_id = "")),
        bundle_id_errors("apple_macos_app", ""),
    ))
    checks.append(expect_equal(
        "flat bundle id fails",
        macos_app_errors(_app(bundle_id = "ledger")),
        ["apple_macos_app: bundle_id 'ledger' is not reverse-DNS: want like 'com.example.app'"],
    ))
    checks.append(expect_equal(
        "freeform version fails",
        macos_app_errors(_app(version = "one")),
        bundle_version_errors("apple_macos_app", "one"),
    ))
    checks.append(expect_equal(
        "no srcs and no library fails",
        macos_app_errors(_app(library = "", srcs = [])),
        ["apple_macos_app: srcs or library is required; pass the native entry point or a Rust static library"],
    ))
    checks.append(expect_equal("srcs alone is clean", macos_app_errors(_app(library = "")), []))
    checks.append(expect_equal(
        "no deployment fails",
        macos_app_errors(_app(deployment = "")),
        ["apple_macos_app: deployment names the minimum macOS version"],
    ))
    checks.append(expect_equal(
        "freeform deployment fails",
        macos_app_errors(_app(deployment = "sonoma")),
        ["apple_macos_app: deployment 'sonoma' is not a dotted version"],
    ))
    checks.append(expect_equal(
        "deployment below the floor fails",
        macos_app_errors(_app(deployment = "13.0")),
        ["apple_macos_app: deployment '13.0' is below 14.0"],
    ))
    checks.append(expect_equal(
        "deployment above the ceiling fails",
        macos_app_errors(_app(deployment = "16.0")),
        ["apple_macos_app: deployment '16.0' is above the Xcode 16 'macosx' ceiling 15.0"],
    ))
    checks.append(expect_equal(
        "x86_64 fails",
        macos_app_errors(_app(arch = "x86_64")),
        ["apple_macos_app: arch 'x86_64' is outside macos: want arm64"],
    ))
    checks.append(expect_equal(
        "provisioning profile fails",
        macos_app_errors(_app(provisioning_profile = "ledger.provisionprofile")),
        ["apple_macos_app: building signs ad hoc only; signing is a separate operation"],
    ))
    checks.append(expect_equal(
        "entitlements fail",
        macos_app_errors(_app(entitlements = "ledger.entitlements")),
        ["apple_macos_app: building signs ad hoc only; signing is a separate operation"],
    ))
    checks.append(expect_equal(
        "Xcode 15.4 fails",
        macos_app_errors(_app(xcode_version = "15.4")),
        ["apple_macos_app: xcode '15.4' is outside 16"],
    ))
    checks.append(expect_equal(
        "freeform Xcode fails",
        macos_app_errors(_app(xcode_version = "latest")),
        ["apple_macos_app: xcode_version 'latest' is not a dotted version"],
    ))
    checks.append(expect_equal("named plugins are clean", plugin_errors(["Share.appex", "Helper.xpc"]), []))
    checks.append(expect_equal(
        "empty plugin fails",
        plugin_errors(["Share.appex", ""]),
        ["apple_macos_app: plugin names must not be empty"],
    ))
    checks.append(expect_equal(
        "repeated plugin fails",
        plugin_errors(["Share.appex", "Share.appex"]),
        ["apple_macos_app: plugin 'Share.appex' is listed twice"],
    ))
    checks.append(expect_equal(
        "plugin errors reach the app",
        macos_app_errors(_app(plugins = [""])),
        ["apple_macos_app: plugin names must not be empty"],
    ))
    checks.append(expect_equal("same package path is stripped", owner_relative_path("pkg/x/Foo.bundle/a", "pkg"), "x/Foo.bundle/a"))
    checks.append(expect_equal("external repo path is stripped", owner_relative_path("../repo+/pkg/f", "pkg"), "f"))
    checks.append(expect_equal("root package path is kept", owner_relative_path("f", ""), "f"))
    checks.append(expect_equal("other package is not stripped", owner_relative_path("other/f", "pkg"), "other/f"))
    return checks

def apple_macos_app_tests(name):
    """Declares the macOS app selection pin test."""
    starlark_test(
        name = name,
        mode = "load",
        checks = _app_checks(),
    )
