"""Analysis tests proving the sanitizer settings `dx_fuzz_binary_select` picks per platform."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("//core:fuzz.bzl", "DxFuzzBinaryInfo", "dx_fuzz_binary_select")

def _stub_impl(ctx):
    executable = ctx.actions.declare_file(ctx.label.name)
    ctx.actions.write(executable, "", is_executable = True)
    return [DefaultInfo(executable = executable)]

_stub_binary = rule(
    implementation = _stub_impl,
    doc = "An executable that needs no toolchain.",
    executable = True,
)

def _settings_impl(ctx):
    env = analysistest.begin(ctx)
    info = analysistest.target_under_test(env)[DxFuzzBinaryInfo]
    asserts.equals(env, ctx.attr.want_asan, info.asan)
    asserts.equals(env, ctx.attr.want_fuzzer, info.fuzzer)
    return analysistest.end(env)

_attrs = {"want_asan": attr.bool(), "want_fuzzer": attr.bool()}

_windows_test = analysistest.make(
    _settings_impl,
    attrs = _attrs,
    config_settings = {"//command_line_option:platforms": str(Label("//core/tests:fuzz_windows_platform"))},
)
_linux_test = analysistest.make(
    _settings_impl,
    attrs = _attrs,
    config_settings = {"//command_line_option:platforms": str(Label("//core/tests:fuzz_linux_platform"))},
)

def fuzz_binary_tests(name):
    """Declares the dx_fuzz_binary_select platform tests."""
    _stub_binary(name = name + "_stub", testonly = True)
    dx_fuzz_binary_select(name = name + "_subject", binary = ":" + name + "_stub")
    _windows_test(name = name + "_windows_test", target_under_test = ":" + name + "_subject", want_asan = False, want_fuzzer = False)
    _linux_test(name = name + "_linux_test", target_under_test = ":" + name + "_subject", want_asan = True, want_fuzzer = True)
    native.test_suite(name = name, tests = [":" + name + "_windows_test", ":" + name + "_linux_test"])
