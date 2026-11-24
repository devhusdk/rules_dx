"""Xcode license gate coverage."""

load("//apple/private/xcode:repos.bzl", "HUB_NAME", "LICENSE_URL", "UNACCEPTED", "hub_build")
load("//core:license.bzl", "APPLE_XCODE")
load("//core/private/testing:defs.bzl", "expect_contains", "expect_equal", "expect_true", "starlark_test")

def _checks():
    accepted = hub_build(True)
    unaccepted = hub_build(False)
    return [
        expect_equal("hub name", HUB_NAME, "dx_apple_xcode"),
        expect_contains("accepted hub declares the gate", accepted, 'name = "license"'),
        expect_contains("accepted hub has an empty error", accepted, 'error = ""'),
        expect_contains("unaccepted hub carries the message", unaccepted, json.encode(UNACCEPTED)),
        expect_contains("message names the id", UNACCEPTED, 'dx_licenses.accept(ids = ["' + APPLE_XCODE + '"])'),
        expect_contains("message names the agreement", UNACCEPTED, LICENSE_URL),
        expect_true("hubs are public", 'visibility = ["//visibility:public"]' in accepted and 'visibility = ["//visibility:public"]' in unaccepted),
    ]

def apple_license_tests(name, size):
    """Declares the Xcode license gate checks."""
    starlark_test(
        name = name,
        size = size,
        mode = "load",
        checks = _checks(),
    )
