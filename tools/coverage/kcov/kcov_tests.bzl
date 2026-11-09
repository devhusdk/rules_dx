"""Analysis tests that a native test runs under kcov on the platforms it names, and nowhere else."""

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

def _platform_test(platform):
    return analysistest.make(
        _kcov_env_impl,
        attrs = _ATTRS,
        config_settings = _COVERAGE | {"//command_line_option:platforms": str(Label(platform))},
    )

# An analysis rule fixes its platform when it is defined, so every platform a caller can name is defined here.
_fortran_linux_test = _platform_test("//fortran/rules:linux_x86_64")
_zig_linux_test = _platform_test("//zig/rules:kcov_linux_x86_64")
_zig_macos_test = _platform_test("//zig/rules:kcov_macos_arm64")

_PLATFORM_TESTS = {
    "//fortran/rules:linux_x86_64": _fortran_linux_test,
    "//zig/rules:kcov_linux_x86_64": _zig_linux_test,
    "//zig/rules:kcov_macos_arm64": _zig_macos_test,
}

_off_test = analysistest.make(
    _kcov_env_impl,
    attrs = _ATTRS,
    config_settings = {"//command_line_option:collect_code_coverage": False},
)

def _platform_rule(platform):
    if platform not in _PLATFORM_TESTS:
        fail("dx_kcov_env_test has no analysis rule for {}; add it to kcov_tests.bzl".format(platform))
    return _PLATFORM_TESTS[platform]

def dx_kcov_env_test(name, target, upstream, instrumented, linux_platform, other_platforms = []):
    """Asserts a test runs under kcov on linux_platform with coverage on, and not on other_platforms."""
    _platform_rule(linux_platform)(name = name, size = "small", target_under_test = target, kcov = True, upstream = upstream, instrumented = instrumented)
    for platform in other_platforms:
        _platform_rule(platform)(name = name + "_" + Label(platform).name, size = "small", target_under_test = target, kcov = False)
    _off_test(name = name + "_off", size = "small", target_under_test = target, kcov = False)
