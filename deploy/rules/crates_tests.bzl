"""Unit and analysis tests for the crates.io publisher."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("//core:defs.bzl", "expect_equal", "starlark_test")
load(":validate_crates.bzl", "crates_name_error", "crates_version_error")

def crates_unit_tests(name):
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal(
                "crates_name_error accepts the demo crate name",
                crates_name_error("crates_demo"),
                "",
            ),
            expect_equal(
                "crates_name_error accepts dashed names",
                crates_name_error("dx-plugin"),
                "",
            ),
            expect_equal(
                "crates_name_error rejects an empty name",
                crates_name_error(""),
                "crates_deploy: invalid crate name '': want a non-empty " +
                "name (for example 'crates_demo')",
            ),
            expect_equal(
                "crates_name_error rejects dots in crate names",
                crates_name_error("dx.plugin"),
                "crates_deploy: invalid crate name 'dx.plugin': want " +
                "only [A-Za-z0-9_-]",
            ),
            expect_equal(
                "crates_name_error rejects name characters",
                crates_name_error("dx;curl evil"),
                "crates_deploy: invalid crate name 'dx;curl evil': want " +
                "only [A-Za-z0-9_-]",
            ),
            expect_equal(
                "crates_version_error accepts the demo version",
                crates_version_error("0.0.0"),
                "",
            ),
            expect_equal(
                "crates_version_error accepts prerelease versions",
                crates_version_error("1.2.3-alpha.1"),
                "",
            ),
            expect_equal(
                "crates_version_error rejects an empty version",
                crates_version_error(""),
                "crates_deploy: invalid version '': want a non-empty " +
                "version (for example '0.0.0')",
            ),
            expect_equal(
                "crates_version_error rejects version characters",
                crates_version_error("1.0;curl evil"),
                "crates_deploy: invalid version '1.0;curl evil': want " +
                "only [A-Za-z0-9._-+]",
            ),
        ],
    )

def _write_content(env):
    writes = [a for a in analysistest.target_actions(env) if a.mnemonic == "FileWrite"]
    asserts.equals(env, 1, len(writes), "want one FileWrite action")
    return writes[0].content if writes else ""

def _fail_config_test_impl(ctx):
    env = analysistest.begin(ctx)
    content = _write_content(env)
    asserts.true(env, "\"api_compat\":\"fail\"" in content, "config must default api_compat to fail: " + content)
    asserts.true(env, "\"api_baseline\":\"registry\"" in content, "config must default api_baseline to registry: " + content)
    return analysistest.end(env)

_fail_config_test = analysistest.make(_fail_config_test_impl)

def _warn_config_test_impl(ctx):
    env = analysistest.begin(ctx)
    content = _write_content(env)
    asserts.true(env, "\"api_compat\":\"warn\"" in content, "config must carry api_compat warn: " + content)
    return analysistest.end(env)

_warn_config_test = analysistest.make(_warn_config_test_impl)

def crates_config_content_tests(name):
    """Checks that the crates config carries the api_compat mode as written."""
    _fail_config_test(
        name = name + "_fail",
        size = "small",
        target_under_test = ":crates_demo_config",
    )
    _warn_config_test(
        name = name + "_warn",
        size = "small",
        target_under_test = ":crates_warn_config",
    )
    native.test_suite(
        name = name,
        tests = [name + "_fail", name + "_warn"],
    )
