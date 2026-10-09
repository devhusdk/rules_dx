"""Analysis proofs for the declared ruff tool override."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("//libs/starlark:canonical.bzl", "strip_canonical")
load("//libs/starlark:defs.bzl", "DxSubjectInfo")
load("//quality:real_aspects.bzl", "real_format_aspect", "real_lint_aspect")

_CANONICAL = "@@"  # buildifier: disable=canonical-repository

_TOOL_FLAG = "//config:tool_ruff"
_TOOL_SETTING = _CANONICAL + "//config:tool_ruff"
_ALTERNATE = _CANONICAL + "//quality/testdata:alternate_ruff"
_ALTERNATE_SHORT = "quality/testdata/alternate_ruff_bin"
_PAIR = _CANONICAL + "//quality/testdata:two_ruffs"
_EMPTY = _CANONICAL + "//quality/testdata:no_ruff"
_POLICY = _CANONICAL + "//quality:fixture_policy"

def _tool_override_subject_impl(ctx):
    """Exposes the resolved ruff binary path beside the real aspect results."""
    fields = {
        "label": strip_canonical(str(ctx.attr.target.label)),
        "ruff": ctx.file._ruff.short_path,
    }
    return [DefaultInfo(files = depset([])), DxSubjectInfo(fields = fields)]

tool_override_subject = rule(
    implementation = _tool_override_subject_impl,
    attrs = {
        "_ruff": attr.label(
            default = _TOOL_FLAG,
            allow_single_file = True,
        ),
        "target": attr.label(
            aspects = [real_lint_aspect, real_format_aspect],
            mandatory = True,
        ),
    },
)

def _subject_under_test(env):
    """Unwraps the possibly transitioned target under test."""
    dep = env.ctx.attr.target_under_test
    return dep[0] if type(dep) == "list" else dep

def _default_impl(ctx):
    """Asserts the managed ruff stays selected without the override flag."""
    env = analysistest.begin(ctx)
    ruff = _subject_under_test(env)[DxSubjectInfo].fields["ruff"]
    asserts.true(env, "alternate_ruff" not in ruff, "default ruff must stay managed, got " + ruff)
    asserts.true(env, ruff.split("/")[-1] in ["ruff", "ruff.exe"], "default ruff must be the managed binary, got " + ruff)
    return analysistest.end(env)

_tool_override_default_test = analysistest.make(_default_impl)

def _selected_impl(ctx):
    """Asserts the flag selects the declared alternate ruff binary."""
    env = analysistest.begin(ctx)
    ruff = _subject_under_test(env)[DxSubjectInfo].fields["ruff"]
    asserts.true(env, ruff == _ALTERNATE_SHORT, "want the declared alternate ruff, got " + ruff)
    return analysistest.end(env)

_tool_override_selected_test = analysistest.make(
    _selected_impl,
    config_settings = {_TOOL_SETTING: _ALTERNATE},
)

def _rejected_impl(ctx):
    """Asserts the expected override rejection diagnostic."""
    env = analysistest.begin(ctx)
    asserts.expect_failure(env, ctx.attr.expected_failure_substring)
    return analysistest.end(env)

_tool_override_rejects_pair_test = analysistest.make(
    _rejected_impl,
    expect_failure = True,
    attrs = {"expected_failure_substring": attr.string(mandatory = True)},
    config_settings = {_TOOL_SETTING: _PAIR},
)

_tool_override_rejects_empty_test = analysistest.make(
    _rejected_impl,
    expect_failure = True,
    attrs = {"expected_failure_substring": attr.string(mandatory = True)},
    config_settings = {_TOOL_SETTING: _EMPTY},
)

_tool_override_rejects_policy_test = analysistest.make(
    _rejected_impl,
    expect_failure = True,
    attrs = {"expected_failure_substring": attr.string(mandatory = True)},
    config_settings = {_TOOL_SETTING: _POLICY},
)

def tool_override_tests(name, subject):
    """Instantiates the ruff override selection and rejection proofs."""
    _tool_override_default_test(
        name = name + "_default_test",
        size = "small",
        target_under_test = subject,
    )
    _tool_override_selected_test(
        name = name + "_selected_test",
        size = "small",
        target_under_test = subject,
    )
    _tool_override_rejects_pair_test(
        name = name + "_rejects_pair_test",
        size = "small",
        target_under_test = subject,
        expected_failure_substring = "two_ruffs' must produce a single file",
    )
    _tool_override_rejects_empty_test(
        name = name + "_rejects_empty_test",
        size = "small",
        target_under_test = subject,
        expected_failure_substring = "no_ruff' must produce a single file",
    )
    _tool_override_rejects_policy_test(
        name = name + "_rejects_policy_test",
        size = "small",
        target_under_test = subject,
        expected_failure_substring = "fixture_policy' must produce a single file",
    )
