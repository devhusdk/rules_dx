"""Analysis test that Windows arm64 resolves the hermetic_launcher stub toolchains."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")

_TEMPLATE = "@hermetic_launcher//launcher:template_toolchain_type"
_FINALIZER = "@hermetic_launcher//launcher:finalizer_toolchain_type"

_ProbeInfo = provider(doc = "Resolved launcher stub files.", fields = ["template", "finalizer"])

def _probe_impl(ctx):
    return [_ProbeInfo(
        template = ctx.toolchains[_TEMPLATE].templatetoolchaininfo.template_exe,
        finalizer = ctx.toolchains[_FINALIZER].finalizer_info.finalizer,
    )]

_probe = rule(implementation = _probe_impl, toolchains = [_TEMPLATE, _FINALIZER])

def _impl(ctx):
    env = analysistest.begin(ctx)
    info = analysistest.target_under_test(env)[_ProbeInfo]
    template = info.template.owner.workspace_name
    finalizer = info.finalizer.owner.workspace_name
    asserts.true(env, template.endswith("runfiles_stub_aarch64_windows"), "template stub from " + template)
    asserts.true(env, finalizer.endswith("finalize_stub_aarch64_windows"), "finalizer stub from " + finalizer)
    return analysistest.end(env)

_WINDOWS_ARM64 = [str(Label("//tools/platforms:windows_arm64"))]

_windows_arm64_test = analysistest.make(
    _impl,
    config_settings = {
        "//command_line_option:platforms": _WINDOWS_ARM64,
        "//command_line_option:extra_execution_platforms": _WINDOWS_ARM64,
    },
)

def hermetic_launcher_tests(name):
    """Instantiates the Windows arm64 launcher toolchain analysis test."""
    _probe(name = name + "_subject", tags = ["manual"])
    _windows_arm64_test(name = name + "_test", target_under_test = ":" + name + "_subject")
    native.test_suite(name = name, tests = [":" + name + "_test"])
