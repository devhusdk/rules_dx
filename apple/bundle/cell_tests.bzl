"""Analysis checks for library cells and ad hoc signing under explicit Apple configurations."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("@bazel_skylib//rules:common_settings.bzl", "BuildSettingInfo")
load("@rules_cc//cc/common:cc_info.bzl", "CcInfo")
load(":defs.bzl", "adhoc_signing_transition")

_XCODES = str(Label("//apple/provisioning:managed_xcodes"))

_IOS_SIM = str(Label("@apple_support//platforms:ios_sim_arm64"))

_IOS_DEVICE = str(Label("@apple_support//platforms:ios_arm64"))

_MACOS = str(Label("@apple_support//platforms:macos_arm64"))

_SIGNING_CERTIFICATE = str(Label("@rules_apple//apple/build_settings:signing_certificate_name"))

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

_macos_test = analysistest.make(
    _impl,
    expect_failure = True,
    attrs = _ATTRS,
    config_settings = {
        "//command_line_option:macos_minimum_os": "14.0",
        "//command_line_option:platforms": [_MACOS],
        "//command_line_option:xcode_version_config": _XCODES,
    },
)

_macos_no_xcode_test = analysistest.make(
    _impl,
    expect_failure = True,
    attrs = _ATTRS,
    config_settings = {
        "//command_line_option:macos_minimum_os": "14.0",
        "//command_line_option:platforms": [_MACOS],
    },
)

_RULES = {
    "device": _device_test,
    "host": _host_test,
    "macos": _macos_test,
    "macos_no_xcode": _macos_no_xcode_test,
    "simulator": _simulator_test,
    "simulator_no_xcode": _simulator_no_xcode_test,
}

def cell_failure_test(name, target, config, expected_failure_substring, **kwargs):
    """Asserts one library cell fails analysis under a named configuration."""
    kwargs.setdefault("size", "small")
    _RULES[config](
        name = name,
        target_under_test = target,
        expected_failure_substring = expected_failure_substring,
        **kwargs
    )

_SigningProbeInfo = provider(
    doc = "The signing identity one configuration resolves.",
    fields = {"identity": "The signing_certificate_name value."},
)

def _signing_probe_impl(ctx):
    return [_SigningProbeInfo(identity = ctx.attr._identity[BuildSettingInfo].value)]

signing_probe = rule(
    implementation = _signing_probe_impl,
    doc = "Reports the rules_apple signing identity of its configuration.",
    attrs = {"_identity": attr.label(default = _SIGNING_CERTIFICATE)},
)

def _signing_probe_device_impl(ctx):
    probe = ctx.attr.probe[0] if type(ctx.attr.probe) == "list" else ctx.attr.probe
    return [probe[_SigningProbeInfo]]

signing_probe_device = rule(
    implementation = _signing_probe_device_impl,
    doc = "Reads one signing probe under the ad hoc signing transition.",
    attrs = {"probe": attr.label(doc = "The signing probe.", cfg = adhoc_signing_transition, mandatory = True)},
)

def _adhoc_signing_impl(ctx):
    env = analysistest.begin(ctx)
    identity = analysistest.target_under_test(env)[_SigningProbeInfo].identity
    asserts.equals(env, "-", identity)
    return analysistest.end(env)

_adhoc_signing_test = analysistest.make(
    _adhoc_signing_impl,
    config_settings = {_SIGNING_CERTIFICATE: "Apple Development"},
)

def adhoc_signing_test(name, target_under_test, **kwargs):
    """Asserts apps sign ad hoc even when a signing identity is set."""
    kwargs.setdefault("size", "small")
    _adhoc_signing_test(name = name, target_under_test = target_under_test, **kwargs)
