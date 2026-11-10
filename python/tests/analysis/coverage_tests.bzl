"""Analysis test that python sources are instrumented under coverage."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")

def _instrumented_impl(ctx):
    env = analysistest.begin(ctx)
    target = analysistest.target_under_test(env)
    names = [f.basename for f in target[InstrumentedFilesInfo].instrumented_files.to_list()]
    asserts.true(env, "catalog.py" in names, "catalog.py not instrumented: %s" % names)
    return analysistest.end(env)

instrumented_sources_test = analysistest.make(
    _instrumented_impl,
    config_settings = {
        "//command_line_option:collect_code_coverage": True,
        "//command_line_option:instrumentation_filter": "python/tests/fixtures/python-service",
    },
)
