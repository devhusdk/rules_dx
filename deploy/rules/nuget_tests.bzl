"""Unit and analysis tests for the NuGet publisher."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load(":api_compat.bzl", "api_compat_error")
load(":nuget.bzl", "NUGET_DEFAULT_REGISTRY_URL", "nuget_id_error", "nuget_nupkg_error", "nuget_registry_url_error", "nuget_version_error")

def nuget_unit_tests(name):
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal(
                "nuget_id_error accepts the demo package id",
                nuget_id_error("nuget_demo"),
                "",
            ),
            expect_equal(
                "nuget_id_error accepts dotted ids",
                nuget_id_error("Dx.Plugin"),
                "",
            ),
            expect_equal(
                "nuget_id_error rejects an empty id",
                nuget_id_error(""),
                "nuget_deploy: invalid package id '': want a non-empty " +
                "id (for example 'nuget_demo')",
            ),
            expect_equal(
                "nuget_id_error rejects id characters",
                nuget_id_error("dx;curl evil"),
                "nuget_deploy: invalid package id 'dx;curl evil': want " +
                "only [A-Za-z0-9._-]",
            ),
            expect_equal(
                "nuget_version_error accepts the demo version",
                nuget_version_error("0.0.0"),
                "",
            ),
            expect_equal(
                "nuget_version_error accepts prerelease versions",
                nuget_version_error("1.2.3-alpha.1"),
                "",
            ),
            expect_equal(
                "nuget_version_error rejects an empty version",
                nuget_version_error(""),
                "nuget_deploy: invalid version '': want a non-empty " +
                "version (for example '0.0.0')",
            ),
            expect_equal(
                "nuget_version_error rejects version characters",
                nuget_version_error("1.0;curl evil"),
                "nuget_deploy: invalid version '1.0;curl evil': want " +
                "only [A-Za-z0-9._-+]",
            ),
            expect_equal(
                "nuget_nupkg_error accepts a pinned nupkg filename",
                nuget_nupkg_error("nuget_demo.0.0.0.nupkg"),
                "",
            ),
            expect_equal(
                "nuget_nupkg_error rejects a non-nupkg filename",
                nuget_nupkg_error("nuget_demo.0.0.0.zip"),
                "nuget_deploy: invalid nupkg 'nuget_demo.0.0.0.zip': want a " +
                "filename ending in .nupkg",
            ),
            expect_equal(
                "nuget_registry_url_error accepts the default registry_url",
                nuget_registry_url_error(NUGET_DEFAULT_REGISTRY_URL),
                "",
            ),
            expect_equal(
                "nuget_registry_url_error rejects plaintext URLs",
                nuget_registry_url_error("http://example.com/v3/index.json"),
                "nuget_deploy: invalid registry_url 'http://example.com/v3/index.json': " +
                "want an https URL so pushes never go over plaintext",
            ),
            expect_equal(
                "nuget_registry_url_error rejects an empty registry_url",
                nuget_registry_url_error(""),
                "nuget_deploy: invalid registry_url '': want a non-empty https " +
                "URL (for example '" + NUGET_DEFAULT_REGISTRY_URL + "')",
            ),
            expect_equal(
                "nuget_deploy rejects an unknown api_compat",
                api_compat_error("nuget_deploy", "loud", "registry"),
                "nuget_deploy: invalid api_compat 'loud': want 'off', 'warn' or 'fail'",
            ),
            expect_equal(
                "nuget_deploy rejects an unknown api_baseline",
                api_compat_error("nuget_deploy", "fail", "tag"),
                "nuget_deploy: invalid api_baseline 'tag': want 'registry' or 'git'",
            ),
        ],
    )

def _config_content_test_impl(ctx):
    env = analysistest.begin(ctx)
    writes = [a for a in analysistest.target_actions(env) if a.mnemonic == "FileWrite"]
    asserts.equals(env, 1, len(writes), "want one FileWrite action")
    content = writes[0].content if writes else ""
    asserts.true(env, "\"api_compat\":\"fail\"" in content, "config must default api_compat to fail: " + content)
    asserts.true(env, "\"api_baseline\":\"registry\"" in content, "config must default api_baseline to registry: " + content)
    asserts.true(env, "Microsoft.DotNet.ApiCompat.Tool.dll" in content, "config must carry the ApiCompat dll: " + content)
    return analysistest.end(env)

_config_content_test = analysistest.make(_config_content_test_impl)

def nuget_config_content_tests(name):
    """Checks that the nuget config defaults api_compat to fail against the registry."""
    _config_content_test(
        name = name,
        size = "small",
        target_under_test = ":nuget_demo_config",
    )
