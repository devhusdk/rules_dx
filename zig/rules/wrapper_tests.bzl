"""Coverage analysis tests for Zig test wrappers."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")

def _kcov_env_impl(ctx):
    env = analysistest.begin(ctx)
    target = analysistest.target_under_test(env)
    run_env = target[RunEnvironmentInfo].environment if RunEnvironmentInfo in target else {}
    if ctx.attr.kcov:
        upstream = run_env.get("DX_KCOV_UPSTREAM", "")
        asserts.true(env, upstream.endswith("/" + ctx.attr.upstream), "upstream is " + upstream)
        asserts.true(env, run_env.get("DX_KCOV_SOURCES", "").endswith(".kcov_sources"), "no kcov sources manifest")
        instrumented = [f.short_path for f in target[InstrumentedFilesInfo].instrumented_files.to_list()]
        asserts.equals(env, ctx.attr.instrumented, instrumented)
    else:
        asserts.false(env, "DX_KCOV_UPSTREAM" in run_env, "kcov env set: " + str(run_env))
    return analysistest.end(env)

_ATTRS = {
    "instrumented": attr.string_list(),
    "kcov": attr.bool(),
    "upstream": attr.string(),
}

_COVERAGE = {
    "//command_line_option:collect_code_coverage": True,
    "//command_line_option:instrumentation_filter": "^//",
}

_linux_test = analysistest.make(
    _kcov_env_impl,
    attrs = _ATTRS,
    config_settings = _COVERAGE | {"//command_line_option:platforms": str(Label("//zig/rules:kcov_linux_x86_64"))},
)

_macos_test = analysistest.make(
    _kcov_env_impl,
    attrs = _ATTRS,
    config_settings = _COVERAGE | {"//command_line_option:platforms": str(Label("//zig/rules:kcov_macos_arm64"))},
)

_off_test = analysistest.make(
    _kcov_env_impl,
    attrs = _ATTRS,
    config_settings = {"//command_line_option:collect_code_coverage": False},
)

def zig_kcov_env_test(name, target, upstream, instrumented):
    """Asserts a Zig test runs under kcov with coverage on Linux, and only then."""
    _linux_test(name = name, size = "small", target_under_test = target, kcov = True, upstream = upstream, instrumented = instrumented)
    _macos_test(name = name + "_macos", size = "small", target_under_test = target, kcov = False)
    _off_test(name = name + "_off", size = "small", target_under_test = target, kcov = False)
