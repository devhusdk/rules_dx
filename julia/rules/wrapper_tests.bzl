"""Analysis tests for Julia coverage on test forwarders."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")

def _config_content(env):
    for action in analysistest.target_actions(env):
        for output in action.outputs.to_list():
            if output.basename.endswith(".julia_coverage.cfg"):
                return action.content
    return None

def _coverage_impl(ctx):
    env = analysistest.begin(ctx)
    target = analysistest.target_under_test(env)
    run_env = target[RunEnvironmentInfo].environment if RunEnvironmentInfo in target else {}
    content = _config_content(env)
    if ctx.attr.expect_runner:
        asserts.true(env, "DX_JULIA_COVERAGE_CONFIG" in run_env, "DX_JULIA_COVERAGE_CONFIG missing")
        asserts.true(env, content != None, "no coverage config written")
        if content != None:
            asserts.true(env, "source\t" + ctx.attr.source + "\n" in content, content)
            asserts.true(env, "upstream\t" + (target.label.repo_name or ctx.workspace_name) + "/" in content, content)
    else:
        asserts.false(env, "DX_JULIA_COVERAGE_CONFIG" in run_env, "coverage env set without coverage")
        asserts.equals(env, None, content)
    return analysistest.end(env)

_coverage_on_test = analysistest.make(
    _coverage_impl,
    attrs = {"expect_runner": attr.bool(default = True), "source": attr.string()},
    config_settings = {"//command_line_option:collect_code_coverage": True},
)

_coverage_off_test = analysistest.make(
    _coverage_impl,
    attrs = {"expect_runner": attr.bool(default = False), "source": attr.string()},
    config_settings = {"//command_line_option:collect_code_coverage": False},
)

def julia_coverage_test(name, target, source):
    """Asserts a Julia test gets the coverage config listing source under coverage, and only then."""
    _coverage_on_test(name = name, size = "small", target_under_test = target, source = source)
    _coverage_off_test(name = name + "_off", size = "small", target_under_test = target)
