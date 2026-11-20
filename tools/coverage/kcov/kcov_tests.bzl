"""Analysis tests that a native test runs under kcov or sancov on the platforms it names, and nowhere else."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load(":kcov.bzl", "dx_kcov_attrs", "dx_kcov_test_coverage")

def _kcov_env_impl(ctx):
    env = analysistest.begin(ctx)
    target = analysistest.target_under_test(env)
    run_env = target[RunEnvironmentInfo].environment if RunEnvironmentInfo in target else {}
    if ctx.attr.kcov:
        upstream = run_env.get("DX_KCOV_UPSTREAM", "")
        asserts.true(env, upstream.endswith("/" + ctx.attr.upstream), "upstream is " + upstream)
        asserts.true(env, run_env.get("DX_KCOV_SOURCES", "").endswith(".kcov_sources"), "no kcov sources manifest")
        to_lcov = run_env.get("DX_TO_LCOV", "")
        asserts.true(env, to_lcov.endswith("/cli/runtime/dx_runtime"), "to_lcov is " + to_lcov)
        instrumented = [f.short_path for f in target[InstrumentedFilesInfo].instrumented_files.to_list()]
        asserts.equals(env, ctx.attr.instrumented, instrumented)
    else:
        asserts.false(env, "DX_KCOV_UPSTREAM" in run_env, "kcov env set: " + str(run_env))
    sancov = run_env.get("DX_SANCOV_UPSTREAM", "")
    if ctx.attr.sancov:
        asserts.true(env, sancov.endswith("/" + ctx.attr.upstream), "sancov upstream is " + sancov)
        asserts.equals(env, "sancov", run_env.get("DX_RUNTIME_TOOL", ""))
        runfiles = [f.short_path for f in target[DefaultInfo].default_runfiles.files.to_list()]
        asserts.true(
            env,
            [p for p in runfiles if p.endswith("cli/runtime/dx_runtime")] != [],
            "no dx_runtime in runfiles",
        )
    else:
        asserts.equals(env, "", sancov)
        asserts.false(env, "DX_RUNTIME_TOOL" in run_env, "DX_RUNTIME_TOOL set: " + str(run_env))
    return analysistest.end(env)

_ATTRS = {
    "instrumented": attr.string_list(),
    "kcov": attr.bool(),
    "sancov": attr.bool(),
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

_fortran_linux_test = _platform_test("//fortran/private:linux_x86_64")
_zig_linux_test = _platform_test("//zig/private:kcov_linux_x86_64")
_zig_macos_test = _platform_test("//zig/private:kcov_macos_arm64")

_PLATFORM_TESTS = {
    "//fortran/private:linux_x86_64": _fortran_linux_test,
    "//zig/private:kcov_linux_x86_64": _zig_linux_test,
    "//zig/private:kcov_macos_arm64": _zig_macos_test,
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

def dx_kcov_env_test(name, target, upstream, instrumented, linux_platform, other_platforms = [], sancov_platforms = []):
    """Asserts kcov runs on linux_platform, sancov on sancov_platforms, and neither elsewhere."""
    _platform_rule(linux_platform)(name = name, size = "small", target_under_test = target, kcov = True, upstream = upstream, instrumented = instrumented)
    for platform in other_platforms:
        _platform_rule(platform)(name = name + "_" + Label(platform).name, size = "small", target_under_test = target, kcov = False, sancov = platform in sancov_platforms, upstream = upstream)
    _off_test(name = name + "_off", size = "small", target_under_test = target, kcov = False)

def _no_kcov_probe_impl(ctx):
    coverage = dx_kcov_test_coverage(ctx, ctx.attr.upstream, [])
    return [coverage.env] if coverage else []

# Runs dx_kcov_test_coverage with the kcov a consumer hub serves on a platform with no pin.
_no_kcov_probe = rule(
    implementation = _no_kcov_probe_impl,
    attrs = dx_kcov_attrs() | {
        "upstream": attr.label(executable = True, cfg = "target"),
        "_dx_kcov": attr.label(
            default = Label("//tools/coverage/kcov:none"),
            cfg = "target",
        ),
    },
)

def _no_kcov_env_impl(ctx):
    env = analysistest.begin(ctx)
    target = analysistest.target_under_test(env)
    asserts.false(env, RunEnvironmentInfo in target, "kcov env set: " + str(target[RunEnvironmentInfo].environment if RunEnvironmentInfo in target else {}))
    return analysistest.end(env)

_unpinned_linux_test = analysistest.make(
    _no_kcov_env_impl,
    config_settings = _COVERAGE | {"//command_line_option:platforms": str(Label("//zig/private:kcov_linux_x86_64"))},
)

def dx_kcov_unpinned_test(name, upstream):
    """Asserts a Linux test whose kcov has no executable runs without kcov."""
    _no_kcov_probe(name = name + "_probe", upstream = upstream, testonly = True)
    _unpinned_linux_test(name = name, size = "small", target_under_test = name + "_probe")
