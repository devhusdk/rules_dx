"""Unit tests for the dev hub shim generator."""

load("//libs/starlark:defs.bzl", "expect_contains", "expect_true", "starlark_test")
load(":dev_hub.bzl", "dev_hub_defs")

def _checks():
    forward = dev_hub_defs("dx_npm", True)
    stub = dev_hub_defs("dx_npm", False)
    return [
        expect_contains("forward loads the hub", forward, 'load("@dx_npm//:defs.bzl", _npm_link_all_packages = "npm_link_all_packages")'),
        expect_contains("forward re-exports the macro", forward, "npm_link_all_packages = _npm_link_all_packages"),
        expect_true("stub never loads the hub", "load(" not in stub and "@dx_npm//" not in stub),
        expect_contains("stub accepts every argument", stub, 'def npm_link_all_packages(name = "node_modules", **kwargs):'),
    ]

def dev_hub_tests(name):
    """Declares the dev hub shim test."""
    starlark_test(
        name = name,
        mode = "load",
        checks = _checks(),
    )
