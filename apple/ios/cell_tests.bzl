"""Analysis failures for app library cells under explicit iOS configurations."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("@rules_cc//cc/common:cc_info.bzl", "CcInfo")

_XCODES = str(Label("//apple/provisioning:managed_xcodes"))

_IOS_SIM = str(Label("@apple_support//platforms:ios_sim_arm64"))

_IOS_DEVICE = str(Label("@apple_support//platforms:ios_arm64"))

def _stub_library_impl(_ctx):
    return [CcInfo()]

stub_library = rule(
    implementation = _stub_library_impl,
    doc = "Provides an empty CcInfo with no static archive.",
)

def _impl(ctx):
    env = analysistest.begin(ctx)
    asserts.expect_failure(env, ctx.attr.expected_failure_substring)
    return analysistest.end(env)

_ATTRS = {"expected_failure_substring": attr.string(mandatory = True)}

_host_test = analysistest.make(
    _impl,
    expect_failure = True,
    attrs = _ATTRS,
)

_simulator_test = analysistest.make(
    _impl,
    expect_failure = True,
    attrs = _ATTRS,
    config_settings = {
        "//command_line_option:ios_minimum_os": "17.0",
        "//command_line_option:platforms": [_IOS_SIM],
        "//command_line_option:xcode_version_config": _XCODES,
    },
)

_simulator_no_xcode_test = analysistest.make(
    _impl,
    expect_failure = True,
    attrs = _ATTRS,
    config_settings = {
        "//command_line_option:ios_minimum_os": "17.0",
        "//command_line_option:platforms": [_IOS_SIM],
    },
)

_device_test = analysistest.make(
    _impl,
    expect_failure = True,
    attrs = _ATTRS,
    config_settings = {
        "//command_line_option:ios_minimum_os": "17.0",
        "//command_line_option:platforms": [_IOS_DEVICE],
        "//command_line_option:xcode_version_config": _XCODES,
    },
)

_RULES = {
    "device": _device_test,
    "host": _host_test,
    "simulator": _simulator_test,
    "simulator_no_xcode": _simulator_no_xcode_test,
}

def cell_failure_test(name, target, config, expected_failure_substring, **kwargs):
    """Asserts one app library cell fails analysis under a named configuration."""
    kwargs.setdefault("size", "small")
    _RULES[config](
        name = name,
        target_under_test = target,
        expected_failure_substring = expected_failure_substring,
        **kwargs
    )
