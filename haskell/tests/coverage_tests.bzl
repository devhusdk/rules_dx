"""Analysis tests for hpc LCOV coverage on Haskell test forwarders."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")

def _descriptor(env):
    for action in analysistest.target_actions(env):
        for output in action.outputs.to_list():
            if output.basename.endswith(".launch"):
                return action.content
    return None

def _coverage_impl(ctx):
    env = analysistest.begin(ctx)
    target = analysistest.target_under_test(env)
    descriptor = _descriptor(env)
    exe = target[DefaultInfo].files_to_run.executable
    if ctx.attr.expect_lcov:
        asserts.true(env, descriptor != None, "no coverage descriptor written")
        asserts.equals(env, target.label.name, exe.basename.removesuffix(".exe"))
        lines = (descriptor or "").splitlines()
        asserts.true(env, len(lines) >= 5, descriptor)
        asserts.true(env, lines[0].startswith("program=") and lines[0].removesuffix(".exe").endswith("cli/hpc_coverage/hpc_coverage"), descriptor)
        asserts.true(env, lines[1].startswith("path=") and "greeter_test_upstream" in lines[1] and not lines[1].endswith(".sh"), descriptor)
        asserts.true(env, lines[2].startswith("path=") and lines[2].removesuffix(".exe").endswith("haskell/private/coverage/hpc_lcov_source"), descriptor)
        asserts.true(env, all([line.startswith("path=") and line.endswith(".mix") for line in lines[3:-1]]), descriptor)
        asserts.equals(env, "arg=--", lines[-1], descriptor)
        runfiles = [f.short_path.removesuffix(".exe") for f in target[DefaultInfo].default_runfiles.files.to_list()]
        asserts.true(env, "haskell/private/coverage/hpc_lcov_source" in runfiles, str(runfiles))
        asserts.true(env, "cli/hpc_coverage/hpc_coverage" in runfiles, str(runfiles))
        asserts.true(env, target.label.package + "/" + target.label.name + ".launch" in runfiles, str(runfiles))
    else:
        asserts.equals(env, None, descriptor)
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

def dx_haskell_coverage_tests(name, target, size):
    """Asserts a Haskell test converts hpc data to LCOV only under coverage."""
    _coverage_on_test(name = name + "_on", size = size, target_under_test = target)
    _coverage_off_test(name = name + "_off", size = size, tags = ["no-coverage"], target_under_test = target)
    native.test_suite(name = name, tests = [name + "_on", name + "_off"])
