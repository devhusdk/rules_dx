"""Analysis failures for Apple tooling probes under an explicit Xcode configuration."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")

_XCODES = str(Label("//apple/provisioning:managed_xcodes"))

_IOS_SIM = str(Label("@apple_support//platforms:ios_sim_arm64"))

_MACOS = str(Label("@apple_support//platforms:macos_arm64"))

def _impl(ctx):
    env = analysistest.begin(ctx)
    asserts.expect_failure(env, ctx.attr.expected_failure_substring)
    return analysistest.end(env)

_ATTRS = {"expected_failure_substring": attr.string(mandatory = True)}

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

_simulator_default_os_test = analysistest.make(
    _impl,
    expect_failure = True,
    attrs = _ATTRS,
    config_settings = {
        "//command_line_option:platforms": [_IOS_SIM],
        "//command_line_option:xcode_version_config": _XCODES,
    },
)

_simulator_future_os_test = analysistest.make(
    _impl,
    expect_failure = True,
    attrs = _ATTRS,
    config_settings = {
        "//command_line_option:ios_minimum_os": "19.0",
        "//command_line_option:platforms": [_IOS_SIM],
        "//command_line_option:xcode_version_config": _XCODES,
    },
)

_macos_test = analysistest.make(
    _impl,
    expect_failure = True,
    attrs = _ATTRS,
    config_settings = {
        "//command_line_option:platforms": [_MACOS],
        "//command_line_option:xcode_version_config": _XCODES,
    },
)

_RULES = {
    "macos": _macos_test,
    "simulator": _simulator_test,
    "simulator_default_os": _simulator_default_os_test,
    "simulator_future_os": _simulator_future_os_test,
}

def probe_failure_test(name, target, config, expected_failure_substring, **kwargs):
    """Asserts one probe fails analysis under a named explicit Xcode configuration."""
    kwargs.setdefault("size", "small")
    _RULES[config](
        name = name,
        target_under_test = target,
        expected_failure_substring = expected_failure_substring,
        **kwargs
    )
