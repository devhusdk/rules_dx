"""Analysis tests for the lychee link request output group."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("//core/quality/testdata:all_languages.bzl", "real_lint_aspect")

def _lint_actions(env):
    return [a for a in analysistest.target_under_test(env).actions if a.mnemonic == "DxRealQualityLint"]

def _link_requests_impl(ctx):
    env = analysistest.begin(ctx)
    runs = _lint_actions(env)
    asserts.true(env, len(runs) > 0, "want a lint action")
    groups = analysistest.target_under_test(env)[OutputGroupInfo]
    asserts.true(env, hasattr(groups, "dx_link_requests"), "the lint shard publishes dx_link_requests")
    if hasattr(groups, "dx_link_requests"):
        names = [f.basename for f in groups.dx_link_requests.to_list()]
        asserts.true(env, len(names) > 0, "want a request file")
        for name in names:
            asserts.true(env, name.endswith("-request.json"), "want a runner request, got " + name)
    return analysistest.end(env)

_link_requests_test = analysistest.make(
    _link_requests_impl,
    extra_target_under_test_aspects = [real_lint_aspect],
)

def link_requests_analysis_tests(name, size):
    """Instantiates the link request output group tests as one suite.

    Args:
        name: suite name and test name prefix.
        size: size of each test.
    """
    _link_requests_test(
        name = name + "_markdown",
        size = size,
        target_under_test = "//core/quality/testdata:fixture_real_markdown",
    )
    native.test_suite(name = name, tests = [name + "_markdown"])
