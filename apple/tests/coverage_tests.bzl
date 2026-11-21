"""Analysis checks that Apple bundle links pull in the clang profile runtime only under coverage."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("//apple/private/bundle:defs.bzl", "EXTRA_TOOLCHAINS_SETTING", "apple_extra_toolchains")
load("//apple/private/bundle:pins.bzl", "XCODE_VERSION")

_PROFILE_FLAG = "-fprofile-instr-generate"

_XCODES = str(Label("//apple/private/provisioning:managed_xcodes"))

_APPLE_CELL = {
    EXTRA_TOOLCHAINS_SETTING: apple_extra_toolchains([]),
    "//command_line_option:ios_multi_cpus": ["sim_arm64"],
    "//command_line_option:macos_cpus": ["arm64"],
    "//command_line_option:xcode_version": XCODE_VERSION,
    "//command_line_option:xcode_version_config": _XCODES,
}

def _profiled_links(env):
    links = [action for action in analysistest.target_actions(env) if action.mnemonic == "ObjcLink"]
    asserts.true(env, len(links) > 0, "want at least one ObjcLink action")
    return [link for link in links if _PROFILE_FLAG in link.argv]

def _on_impl(ctx):
    env = analysistest.begin(ctx)
    asserts.true(env, len(_profiled_links(env)) > 0, "want {} on the ObjcLink under coverage".format(_PROFILE_FLAG))
    return analysistest.end(env)

def _off_impl(ctx):
    env = analysistest.begin(ctx)
    asserts.equals(env, 0, len(_profiled_links(env)), "ObjcLink actions with {} outside coverage".format(_PROFILE_FLAG))
    return analysistest.end(env)

_on_test = analysistest.make(
    _on_impl,
    config_settings = _APPLE_CELL | {"//command_line_option:collect_code_coverage": True},
)

_off_test = analysistest.make(
    _off_impl,
    config_settings = _APPLE_CELL | {"//command_line_option:collect_code_coverage": False},
)

def apple_coverage_link_test(name, target_under_test, **kwargs):
    """Asserts one rules_apple bundle links the profile runtime under coverage and not otherwise."""
    kwargs.setdefault("size", "small")
    macos = ["@platforms//os:macos"]
    _on_test(name = name + "_on", target_under_test = target_under_test, target_compatible_with = macos, **kwargs)
    _off_test(name = name + "_off", target_under_test = target_under_test, target_compatible_with = macos, **kwargs)
    native.test_suite(
        name = name,
        tests = [":" + name + "_on", ":" + name + "_off"],
        tags = kwargs.get("tags", []),
    )
