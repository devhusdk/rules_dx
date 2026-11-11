"""Analysis failures for Apple tooling probes under an explicit Xcode configuration."""

load("//core:failure_test.bzl", "failure_test_rule")

_XCODES = str(Label("//apple/private/provisioning:managed_xcodes"))

_IOS_SIM = str(Label("@apple_support//platforms:ios_sim_arm64"))

_MACOS = str(Label("@apple_support//platforms:macos_arm64"))

_simulator_test = failure_test_rule({
        "//command_line_option:ios_minimum_os": "17.0",
        "//command_line_option:platforms": [_IOS_SIM],
        "//command_line_option:xcode_version_config": _XCODES,
    })

_simulator_default_os_test = failure_test_rule({
        "//command_line_option:platforms": [_IOS_SIM],
        "//command_line_option:xcode_version_config": _XCODES,
    })

_simulator_future_os_test = failure_test_rule({
        "//command_line_option:ios_minimum_os": "19.0",
        "//command_line_option:platforms": [_IOS_SIM],
        "//command_line_option:xcode_version_config": _XCODES,
    })

_macos_test = failure_test_rule({
        "//command_line_option:platforms": [_MACOS],
        "//command_line_option:xcode_version_config": _XCODES,
    })

_unconfigured_test = failure_test_rule({
        "//command_line_option:ios_minimum_os": "17.0",
        "//command_line_option:platforms": [_IOS_SIM],
    })

_host_test = failure_test_rule({"//command_line_option:xcode_version_config": _XCODES})

_RULES = {
    "host": _host_test,
    "macos": _macos_test,
    "simulator": _simulator_test,
    "unconfigured": _unconfigured_test,
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
