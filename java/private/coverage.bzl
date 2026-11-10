"""JVM test coverage shared by the java, kotlin and scala wrappers."""

load("//libs/starlark:wrapper.bzl", "dx_forwarded_run_environment")

DX_JVM_TEST_RUNNER = "com.google.testing.junit.runner.BazelTestRunner"

def dx_jvm_test_coverage(ctx, upstream):
    """Sets JACOCO_MAIN_CLASS from the main_class attr for JacocoCoverageRunner."""
    if InstrumentedFilesInfo not in upstream:
        fail("upstream target has no InstrumentedFilesInfo: " + str(upstream.label))
    return struct(
        instrumented = upstream[InstrumentedFilesInfo],
        env = dx_forwarded_run_environment(upstream, {"JACOCO_MAIN_CLASS": ctx.attr.main_class}),
        runfiles = None,
    )
