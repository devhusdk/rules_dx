"""Tests for the dx_terraform extension helpers and terraform_module providers."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load("//terraform:extensions.bzl", "default_url", "provider_errors")
load("//terraform/rules:providers.bzl", "TerraformModuleInfo", "provider_mirror_root")

_SHA = "a" * 64
_SRC = "registry.terraform.io/hashicorp/null"

def provider_unit_tests(name):
    """Declares unit tests of the extension tag validation."""
    starlark_test(name = name, mode = "unit", checks = [
        expect_equal("valid", provider_errors(_SRC, "3.2.4", {"linux_amd64": _SHA}, {}), []),
        expect_equal("bad hex", "64 lowercase hex" in provider_errors(_SRC, "3.2.4", {"linux_amd64": "xyz"}, {})[0], True),
        expect_equal("upper hex", len(provider_errors(_SRC, "3.2.4", {"linux_amd64": "A" * 64}, {})), 1),
        expect_equal("unknown target", "unknown target" in provider_errors(_SRC, "3.2.4", {"plan9_amd64": _SHA}, {})[0], True),
        expect_equal("url needed", len(provider_errors("registry.example.com/acme/x", "1", {"linux_amd64": _SHA}, {})), 1),
        expect_equal("url ok", provider_errors("registry.example.com/acme/x", "1", {"linux_amd64": _SHA}, {"linux_amd64": "https://e.invalid/x.zip"}), []),
        expect_equal("url without sha", len(provider_errors(_SRC, "1", {"linux_amd64": _SHA}, {"linux_arm64": "u"})), 1),
        expect_equal("bad source", len(provider_errors("null", "1", {"linux_amd64": _SHA}, {})) > 0, True),
        expect_equal("no sha", len(provider_errors(_SRC, "1", {}, {})), 1),
        expect_equal(
            "default url",
            default_url(_SRC, "3.2.4", "linux_amd64"),
            "https://releases.hashicorp.com/terraform-provider-null/3.2.4/terraform-provider-null_3.2.4_linux_amd64.zip",
        ),
        expect_equal("mirror root", provider_mirror_root("a/b/" + _SRC + "/3.2.4/linux_amd64/tf", _SRC, "3.2.4", "linux_amd64"), "a/b"),
        expect_equal("mirror root mismatch", provider_mirror_root("a/tf", _SRC, "3.2.4", "linux_amd64"), None),
    ])

def _collects_impl(ctx):
    env = analysistest.begin(ctx)
    infos = analysistest.target_under_test(env)[TerraformModuleInfo].transitive_providers.to_list()
    asserts.equals(env, ctx.attr.expected, sorted([p.version for p in infos]))
    return analysistest.end(env)

collects_providers_test = analysistest.make(
    _collects_impl,
    attrs = {"expected": attr.string_list()},
)
