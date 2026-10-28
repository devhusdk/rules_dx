"""Analysis proof that Vale on reStructuredText or AsciiDoc refuses to run without its converter."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")

def _rst_impl(ctx):
    env = analysistest.begin(ctx)
    asserts.expect_failure(env, "vale: rst files need rst2html; own them with rst_library from @rules_dx//rst/rules:defs.bzl")
    return analysistest.end(env)

def _asciidoc_impl(ctx):
    env = analysistest.begin(ctx)
    asserts.expect_failure(env, "vale: asciidoc files need asciidoctor; own them with asciidoc_library from @rules_dx//asciidoc/rules:defs.bzl")
    return analysistest.end(env)

rst_no_converter_test = analysistest.make(
    _rst_impl,
    expect_failure = True,
    config_settings = {
        "//command_line_option:collect_code_coverage": False,
        str(Label("//config:workspace")): str(Label("//quality/tests/fixtures/consumer_policy:rst_policy")),
    },
)

asciidoc_no_converter_test = analysistest.make(
    _asciidoc_impl,
    expect_failure = True,
    config_settings = {
        "//command_line_option:collect_code_coverage": False,
        str(Label("//config:workspace")): str(Label("//quality/tests/fixtures/consumer_policy:asciidoc_policy")),
    },
)
