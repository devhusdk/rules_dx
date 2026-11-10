"""Analysis tests for hpc LCOV coverage on Haskell test forwarders."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")

def _script(env):
    for action in analysistest.target_actions(env):
        for output in action.outputs.to_list():
            if output.basename.endswith("_upstream_coverage.sh"):
                return action.content
    return None

def _coverage_impl(ctx):
    env = analysistest.begin(ctx)
    target = analysistest.target_under_test(env)
    script = _script(env)
    exe = target[DefaultInfo].files_to_run.executable
    if ctx.attr.expect_lcov:
        asserts.true(env, script != None, "no coverage script written")
        asserts.true(env, exe.basename.endswith(".sh"), exe.basename)
        asserts.true(env, "COVERAGE_DIR/haskell.dat" in (script or ""), script)
        runfiles = [f.short_path for f in target[DefaultInfo].default_runfiles.files.to_list()]
        asserts.true(env, "haskell/private/coverage/hpc_lcov" in runfiles, str(runfiles))
    else:
        asserts.equals(env, None, script)
    return analysistest.end(env)

_coverage_on_test = analysistest.make(
    _coverage_impl,
    attrs = {"expect_lcov": attr.bool(default = True)},
    config_settings = {"//command_line_option:collect_code_coverage": True},
)

_coverage_off_test = analysistest.make(
    _coverage_impl,
    attrs = {"expect_lcov": attr.bool(default = False)},
)

def dx_haskell_coverage_tests(name, target):
    """Asserts a Haskell test converts hpc data to LCOV only under coverage."""
    _coverage_on_test(name = name + "_on", size = "small", target_under_test = target)
    _coverage_off_test(name = name + "_off", size = "small", tags = ["no-coverage"], target_under_test = target)
    native.test_suite(name = name, tests = [name + "_on", name + "_off"])
