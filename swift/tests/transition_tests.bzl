"""Tests for the execution platforms and toolchains Swift targets build with."""

load("@bazel_skylib//lib:unittest.bzl", "asserts", "unittest")
load("//swift/private/platforms:settings.bzl", "swift_platform_settings")

_APPLE = "@@+x+local_config_apple_cc_toolchains//:all"
_LINUX_X86_64 = ["@platforms//cpu:x86_64", "@platforms//os:linux"]
_LINUX_AARCH64 = ["@platforms//cpu:aarch64", "@platforms//os:linux"]
_MACOS = ["@platforms//cpu:aarch64", "@platforms//os:macos"]

def _settings(toolchains = []):
    return {
        "//command_line_option:extra_execution_platforms": [],
        "//command_line_option:extra_toolchains": toolchains,
    }

def _attr(platforms = [], toolchains = []):
    return struct(platforms = platforms, _swift_extra_toolchains = toolchains)

def _run(settings, attr, *host_constraints):
    return swift_platform_settings(settings, attr, attr.platforms, *host_constraints)

def _linux_test_impl(ctx):
    env = unittest.begin(ctx)
    out = _run(_settings(["//a:b"]), _attr(["//swift/private/platforms:exec_linux_x86_64"]))
    asserts.equals(env, ["//a:b"], out["//command_line_option:extra_toolchains"])
    return unittest.end(env)

def _macos_first_test_impl(ctx):
    env = unittest.begin(ctx)
    out = _run(_settings(["//a:b"]), _attr([], [_APPLE]))
    asserts.equals(env, [_APPLE, "//a:b"], out["//command_line_option:extra_toolchains"])
    return unittest.end(env)

def _idempotent_test_impl(ctx):
    env = unittest.begin(ctx)
    attr = _attr([], [_APPLE])
    once = _run(_settings(["//a:b"]), attr)
    twice = _run(once, attr)
    asserts.equals(env, once, twice)
    return unittest.end(env)

def _empty_test_impl(ctx):
    env = unittest.begin(ctx)
    out = _run(_settings(), _attr())
    asserts.equals(env, [], out["//command_line_option:extra_toolchains"])
    return unittest.end(env)

def _cross_test_impl(ctx):
    env = unittest.begin(ctx)
    target = str(Label("//swift/private/platforms:exec_linux_aarch64"))
    host = str(Label("//swift/private/platforms:exec_linux_x86_64"))
    out = _run(_settings(), _attr([target]), _LINUX_X86_64)
    asserts.equals(env, [host], out["//command_line_option:extra_execution_platforms"])
    again = _run(out, _attr([target]), _LINUX_X86_64)
    asserts.equals(env, out, again)
    return unittest.end(env)

def _native_arm64_test_impl(ctx):
    env = unittest.begin(ctx)
    target = str(Label("//swift/private/platforms:exec_linux_aarch64"))
    out = _run(_settings(), _attr([target]), _LINUX_AARCH64)
    asserts.equals(env, [target], out["//command_line_option:extra_execution_platforms"])
    return unittest.end(env)

def _macos_host_test_impl(ctx):
    env = unittest.begin(ctx)
    out = _run(_settings(), _attr(["//a:p"]), _MACOS)
    asserts.equals(env, ["//a:p"], out["//command_line_option:extra_execution_platforms"])
    return unittest.end(env)

_linux_test = unittest.make(_linux_test_impl)
_cross_test = unittest.make(_cross_test_impl)
_native_arm64_test = unittest.make(_native_arm64_test_impl)
_macos_host_test = unittest.make(_macos_host_test_impl)
_macos_first_test = unittest.make(_macos_first_test_impl)
_idempotent_test = unittest.make(_idempotent_test_impl)
_empty_test = unittest.make(_empty_test_impl)

def transition_test_suite(name):
    """Declares the transition tests.

    Args:
        name: The suite name.
    """
    unittest.suite(
        name,
        _linux_test,
        _cross_test,
        _native_arm64_test,
        _macos_host_test,
        _macos_first_test,
        _idempotent_test,
        _empty_test,
    )
