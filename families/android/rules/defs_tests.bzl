"""Tests for android_native_library attribute forwarding and API level selection."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("@rules_dx//core:defs.bzl", "expect_equal", "starlark_test")
load("//toolchain:cc_toolchain_config.bzl", "NdkToolchainLevelInfo")
load(":kwargs.bzl", "android_split_native_kwargs")

def _split_checks():
    shared, rest = android_split_native_kwargs({
        "deps": ["a"],
        "tags": ["t"],
        "target_compatible_with": ["c"],
        "testonly": True,
    })
    none_shared, none_rest = android_split_native_kwargs({"deps": ["a"], "data": ["d"]})
    return [
        expect_equal("shared keys", shared, {"tags": ["t"], "target_compatible_with": ["c"], "testonly": True}),
        expect_equal("rust only keys", rest, {"deps": ["a"]}),
        expect_equal("no shared keys", none_shared, {}),
        expect_equal("all rust only", none_rest, {"deps": ["a"], "data": ["d"]}),
    ]

def _level_impl(ctx):
    env = analysistest.begin(ctx)
    info = analysistest.target_under_test(env)[NdkToolchainLevelInfo]
    asserts.equals(env, ctx.attr.want_level, info.api_level)
    asserts.equals(env, ctx.attr.want_target, info.clang_target)
    return analysistest.end(env)

_attrs = {"want_level": attr.int(), "want_target": attr.string()}

_floor_test = analysistest.make(_level_impl, attrs = _attrs)
_selected_test = analysistest.make(
    _level_impl,
    attrs = _attrs,
    config_settings = {str(Label("//platforms:api_level")): 34},
)

def android_native_library_tests(name):
    """Declares the android_native_library tests."""
    starlark_test(name = name + "_split", mode = "load", checks = _split_checks())
    _floor_test(
        name = name + "_floor_level_test",
        target_under_test = "//toolchain:fake_config",
        want_level = 31,
        want_target = "aarch64-linux-android31",
    )
    _selected_test(
        name = name + "_selected_level_test",
        target_under_test = "//toolchain:fake_config",
        want_level = 34,
        want_target = "aarch64-linux-android34",
    )
    native.test_suite(name = name, tests = [
        ":" + name + "_split",
        ":" + name + "_floor_level_test",
        ":" + name + "_selected_level_test",
    ])
