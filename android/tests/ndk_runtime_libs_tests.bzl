"""Analysis tests for the shared libraries the NDK linker loads."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("//android/private/sdk:repos.bzl", "NDK_PREBUILT", "NDK_RUNTIME_LIBS", "NDK_TOOL_SUFFIX")

def _ld_lld_host(path):
    for host in sorted(NDK_PREBUILT.keys()):
        if path.endswith("/" + NDK_PREBUILT[host] + "/bin/ld.lld" + NDK_TOOL_SUFFIX[host]):
            return host
    return None

def _ld_lld_runtime_libs_impl(ctx):
    env = analysistest.begin(ctx)
    links = []
    for action in analysistest.target_actions(env):
        hosts = [_ld_lld_host(f.path) for f in action.inputs.to_list()]
        hosts = [host for host in hosts if host != None]
        if hosts:
            links.append((action, hosts[0]))
    asserts.true(env, len(links) > 0, "no action stages ld.lld")
    for action, host in links:
        staged = [f.path for f in action.inputs.to_list()]
        for lib in NDK_RUNTIME_LIBS[host]:
            want = "/" + NDK_PREBUILT[host] + "/" + lib
            found = [path for path in staged if path.endswith(want)]
            asserts.true(env, len(found) > 0, "%s stages the %s ld.lld without %s" % (action.mnemonic, host, lib))
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
