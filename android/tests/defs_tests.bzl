"""Tests for android_native_library attribute forwarding and API level selection."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("//android/private/library:kwargs.bzl", "android_split_native_kwargs")
load("//android/private/toolchain:cc_toolchain_config.bzl", "NdkToolchainLevelInfo")
load("//core/private/testing:defs.bzl", "expect_equal", "starlark_test")

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
    config_settings = {str(Label("//android:api_level")): 34},
)

def _ops_launcher_impl(ctx):
    env = analysistest.begin(ctx)
    lines = []
    for action in analysistest.target_actions(env):
        if any([output.basename.endswith(".launch") for output in action.outputs.to_list()]):
            lines = action.content.splitlines()
    asserts.true(env, len(lines) >= 2, str(lines))
    asserts.true(env, lines[0].startswith("program=") and lines[0].removesuffix(".exe").endswith("cli/runtime/dx_runtime"), str(lines))
    asserts.equals(env, "env=DX_RUNTIME_TOOL\tandroid_ops", lines[1])
    asserts.true(env, "arg=adb" in lines, str(lines))
    return analysistest.end(env)

_ops_launcher_test = analysistest.make(_ops_launcher_impl)

def _apk_set_action_impl(ctx):
    env = analysistest.begin(ctx)
    actions = [action for action in analysistest.target_actions(env) if action.mnemonic == "DxApkSet"]
    asserts.equals(env, 1, len(actions))
    argv = actions[0].argv if actions else []
    asserts.true(env, len(argv) >= 3, str(argv))
    asserts.equals(env, "dx", argv[0].replace("\\", "/").split("/")[-1].removesuffix(".exe") if argv else "", str(argv))
    asserts.equals(env, ["android_ops", "apk-set"], argv[1:3], str(argv))
    return analysistest.end(env)

_apk_set_action_test = analysistest.make(_apk_set_action_impl)

def android_ops_tests(name, launcher, apk_set, size = "small"):
    """Checks that adb launchers run dx-runtime's android_ops helper and APK sets run `dx android_ops`."""
    _ops_launcher_test(name = name + "_launcher", size = size, target_under_test = launcher)
    _apk_set_action_test(name = name + "_apk_set", size = size, target_under_test = apk_set)
    native.test_suite(name = name, tests = [":" + name + "_launcher", ":" + name + "_apk_set"])

def android_native_library_tests(name, size = "small"):
    """Declares the android_native_library tests."""
    starlark_test(name = name + "_split", size = size, mode = "load", checks = _split_checks())
    _floor_test(
        name = name + "_floor_level_test",
        size = size,
        target_under_test = "//android/private/toolchain:fake_config",
        want_level = 31,
        want_target = "aarch64-linux-android31",
    )
    _selected_test(
        name = name + "_selected_level_test",
        size = size,
        target_under_test = "//android/private/toolchain:fake_config",
        want_level = 34,
        want_target = "aarch64-linux-android34",
    )
    native.test_suite(name = name, tests = [
        ":" + name + "_split",
        ":" + name + "_floor_level_test",
        ":" + name + "_selected_level_test",
    ])
