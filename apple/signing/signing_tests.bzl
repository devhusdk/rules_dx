"""macOS signing bundle checks."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load(":defs.bzl", "macos_bundle_errors")

def _checks():
    kind = "apple_macos_sign"
    label = "//app:ledger"
    not_app = ["apple_macos_sign: '//app:ledger' is not a macOS application"]
    return [
        expect_equal("macOS app is clean", macos_bundle_errors(kind, label, "macos", ".app", "com.example.ledger"), []),
        expect_equal("iOS app fails", macos_bundle_errors(kind, label, "ios", ".app", "com.example.ledger"), not_app),
        expect_equal("macOS framework fails", macos_bundle_errors(kind, label, "macos", ".framework", "com.example.ledger"), not_app),
        expect_equal("missing extension fails", macos_bundle_errors(kind, label, "macos", "", "com.example.ledger"), not_app),
        expect_equal(
            "flat bundle id fails",
            macos_bundle_errors(kind, label, "macos", ".app", "ledger"),
            ["apple_macos_sign: bundle_id 'ledger' is not reverse-DNS: want like 'com.example.app'"],
        ),
        expect_equal(
            "notarize names itself",
            macos_bundle_errors("apple_macos_notarize", label, "tvos", ".app", "com.example.ledger"),
            ["apple_macos_notarize: '//app:ledger' is not a macOS application"],
        ),
    ]

def apple_signing_tests(name):
    """Declares the macOS signing bundle check test."""
    starlark_test(
        name = name,
        mode = "load",
        checks = _checks(),
    )
