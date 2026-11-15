"""Tests for the terraform extension helpers and terraform_library providers."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("//core:defs.bzl", "expect_equal", "starlark_test")
load("//terraform:extensions.bzl", "PLATFORM_TARGETS", "TARGETS", "hub_build", "hub_providers", "provider_repos")
load("//terraform/private:providers.bzl", "TerraformModuleInfo", "provider_mirror_root")

_SHA = "a" * 64
_SRC = "registry.terraform.io/hashicorp/null"

def provider_unit_tests(name):
    """Declares unit tests of the lock to hub translation."""
    lock = {"packages": [{
        "archives": [
            {"platform": "linux_x86_64", "sha256": _SHA, "urls": ["https://e.invalid/l.zip"]},
            {"platform": "macos_arm64", "sha256": _SHA, "urls": ["https://e.invalid/m.zip"]},
        ],
        "name": _SRC,
        "version": "3.2.4",
    }]}
    starlark_test(name = name, mode = "unit", checks = [
        expect_equal("repos", sorted(provider_repos("hub", lock).keys()), ["hub_null_darwin_arm64", "hub_null_linux_amd64"]),
        expect_equal("repo attributes", provider_repos("hub", lock)["hub_null_linux_amd64"], {
            "sha256": _SHA,
            "source": _SRC,
            "target": "linux_amd64",
            "urls": ["https://e.invalid/l.zip"],
            "version": "3.2.4",
        }),
        expect_equal("hub entries", hub_providers("hub", lock), {"null": {
            "repos": {"darwin_arm64": "hub_null_darwin_arm64", "linux_amd64": "hub_null_linux_amd64"},
            "source": _SRC,
        }}),
        expect_equal("every platform maps to a target", sorted(PLATFORM_TARGETS.values()), sorted(TARGETS.keys())),
        expect_equal("hub build selects per target", "@hub_null_linux_amd64//:provider" in hub_build(hub_providers("hub", lock)), True),
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
