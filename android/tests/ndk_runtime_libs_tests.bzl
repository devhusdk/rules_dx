"""Analysis tests for the shared libraries the NDK linker loads."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")

def _ld_lld_runtime_libs_impl(ctx):
    env = analysistest.begin(ctx)
    links = [
        action
        for action in analysistest.target_actions(env)
        if [f for f in action.inputs.to_list() if f.basename == "ld.lld"]
    ]
    asserts.true(env, len(links) > 0, "no action stages ld.lld")
    for action in links:
        libs = [f.path for f in action.inputs.to_list() if f.path.endswith("/lib/libxml2.so.2")]
        asserts.true(env, len(libs) > 0, "%s stages ld.lld without lib/libxml2.so.2" % action.mnemonic)
    return analysistest.end(env)

_ld_lld_runtime_libs_test = analysistest.make(
    _ld_lld_runtime_libs_impl,
    config_settings = {
        "//command_line_option:platforms": [str(Label("//android:android_device"))],
    },
)

def ndk_link_stages_runtime_libs_test(name, target_under_test):
    """Asserts every action that stages the NDK ld.lld also stages the libraries it loads."""
    _ld_lld_runtime_libs_test(name = name, size = "small", target_under_test = target_under_test)
