"""Analysis tests for the buf breaking base image input."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("//core/quality/testdata:all_languages.bzl", "real_protobuf_lint_aspect")

_IMAGE = "core/quality/tests/fixtures/buf_breaking/base.binpb"
_POLICY = {str(Label("//config:workspace")): str(Label("//core/quality/tests/fixtures/consumer_policy:shared_policy"))}

def _lint_action(env):
    runs = [a for a in analysistest.target_under_test(env).actions if a.mnemonic == "DxRealQualityLint"]
    asserts.equals(env, 1, len(runs), "want one lint action")
    return runs[0] if len(runs) == 1 else None

def _request(env):
    return "".join([
        a.content
        for a in analysistest.target_under_test(env).actions
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

def _two_images_impl(ctx):
    env = analysistest.begin(ctx)
    asserts.expect_failure(env, "//config:buf_breaking_against must name one image file, got 2")
    return analysistest.end(env)

_with_image_test = analysistest.make(
    _with_image_impl,
    config_settings = _POLICY | {
        str(Label("//config:buf_breaking_against")): str(Label("//core/quality/tests/fixtures/buf_breaking:base.binpb")),
    },
    extra_target_under_test_aspects = [real_protobuf_lint_aspect],
)

_without_image_test = analysistest.make(
    _without_image_impl,
    config_settings = _POLICY,
    extra_target_under_test_aspects = [real_protobuf_lint_aspect],
)

_two_images_test = analysistest.make(
    _two_images_impl,
    config_settings = _POLICY | {
        str(Label("//config:buf_breaking_against")): str(Label("//core/quality/tests/fixtures/buf_breaking:two_images")),
    },
    expect_failure = True,
    extra_target_under_test_aspects = [real_protobuf_lint_aspect],
)

def buf_breaking_analysis_tests(name, size):
    """Instantiates the base image input tests as one suite.

    Args:
        name: suite name and test name prefix.
        size: size of each test.
    """
    subject = "//core/quality/tests/fixtures/consumer_policy:clean_protobuf"
    _with_image_test(name = name + "_with_image", size = size, target_under_test = subject)
    _without_image_test(name = name + "_without_image", size = size, target_under_test = subject)
    _two_images_test(name = name + "_two_images", size = size, target_under_test = subject)
    native.test_suite(name = name, tests = [name + "_with_image", name + "_without_image", name + "_two_images"])
