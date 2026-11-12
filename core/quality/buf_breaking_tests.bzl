"""Analysis tests for the buf breaking base image input."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("//core:failure_test.bzl", "failure_test")
load("//core/quality/testdata:all_languages.bzl", "real_protobuf_lint_aspect")
load("//core/quality/testdata:config_subject.bzl", "config_attr", "config_transition")

BUF_BREAKING_CONFIGS = ["buf_breaking_image", "buf_breaking_two_images", "shared"]

_IMAGE = "core/quality/tests/fixtures/buf_breaking/base.binpb"

_LintActionsInfo = provider(
    doc = "Lint and request actions of the subject under one TEST_CONFIGS entry.",
    fields = {"actions": "Actions of the target with the protobuf lint aspect."},
)

def _subject_impl(ctx):
    target = ctx.attr.target[0]
    return [DefaultInfo(files = target[DefaultInfo].files), _LintActionsInfo(actions = target.actions)]

_subject = rule(
    implementation = _subject_impl,
    doc = "Builds the target under one TEST_CONFIGS entry and forwards its actions.",
    attrs = {
        "config": config_attr(),
        "target": attr.label(doc = "Target built under the entry.", mandatory = True, cfg = config_transition, aspects = [real_protobuf_lint_aspect]),
    },
)

def _actions(env):
    return analysistest.target_under_test(env)[_LintActionsInfo].actions

def _lint_action(env):
    runs = [a for a in _actions(env) if a.mnemonic == "DxRealQualityLint"]
    asserts.equals(env, 1, len(runs), "want one lint action")
    return runs[0] if len(runs) == 1 else None

def _request(env):
    return "".join([
        a.content
        for a in _actions(env)
        if a.mnemonic == "FileWrite" and a.outputs.to_list()[0].basename.endswith("-request.json")
    ])

def _with_image_impl(ctx):
    env = analysistest.begin(ctx)
    run = _lint_action(env)
    if run != None:
        asserts.equals(env, [_IMAGE], [f.short_path for f in run.inputs.to_list() if f.basename == "base.binpb"])
    request = _request(env)
    asserts.true(env, "DX_BUF_BREAKING_AGAINST" in request, "the request names the base image variable")
    asserts.true(env, _IMAGE in request, "the request names the base image path")
    return analysistest.end(env)

def _without_image_impl(ctx):
    env = analysistest.begin(ctx)
    run = _lint_action(env)
    if run != None:
        asserts.equals(env, [], [f.short_path for f in run.inputs.to_list() if f.basename == "base.binpb"])
    asserts.false(env, "DX_BUF_BREAKING_AGAINST" in _request(env), "the request names no base image")
    return analysistest.end(env)

_with_image_test = analysistest.make(_with_image_impl)

_without_image_test = analysistest.make(_without_image_impl)

def buf_breaking_analysis_tests(name, size):
    """Instantiates the base image input tests as one suite.

    Args:
        name: suite name and test name prefix.
        size: size of each test.
    """
    target = "//core/quality/tests/fixtures/consumer_policy:clean_protobuf"
    for suffix, config in [("_with_image", "buf_breaking_image"), ("_without_image", "shared"), ("_two_images", "buf_breaking_two_images")]:
        _subject(name = name + suffix + "_subject", config = config, target = target, testonly = True)
    _with_image_test(name = name + "_with_image", size = size, target_under_test = name + "_with_image_subject")
    _without_image_test(name = name + "_without_image", size = size, target_under_test = name + "_without_image_subject")
    failure_test(name = name + "_two_images", size = size, target = name + "_two_images_subject", expected_failure_substring = "//config:buf_breaking_against must name one image file, got 2")
    native.test_suite(name = name, tests = [name + "_with_image", name + "_without_image", name + "_two_images"])
