"""Analysis tests for the Scala toolchains rules_dx registers."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")

_SCALA_TOOLCHAIN_TYPE = "@rules_scala//scala:toolchain_type"
_TESTING_TOOLCHAIN_TYPE = "@rules_scala//testing/toolchain:testing_toolchain_type"

_StatsFileInfo = provider(
    doc = "Whether the resolved Scala toolchain writes a stats file.",
    fields = {"enabled": "The `enable_stats_file` value."},
)

def _stats_file_impl(ctx):
    return [_StatsFileInfo(enabled = ctx.toolchains[_SCALA_TOOLCHAIN_TYPE].enable_stats_file)]

_stats_file = rule(
    implementation = _stats_file_impl,
    doc = "Returns whether the resolved Scala toolchain writes a stats file.",
    toolchains = [_SCALA_TOOLCHAIN_TYPE],
)

def _stats_file_off_impl(ctx):
    env = analysistest.begin(ctx)
    enabled = analysistest.target_under_test(env)[_StatsFileInfo].enabled
    asserts.false(env, enabled, "Scala toolchain writes a stats file; its compile time makes Scalac outputs differ between builds")
    return analysistest.end(env)

_stats_file_off_2_13_test = analysistest.make(
    _stats_file_off_impl,
    config_settings = {str(Label("//config:toolchains")): True},
)

_stats_file_off_3_test = analysistest.make(
    _stats_file_off_impl,
    config_settings = {
        str(Label("//config:toolchains")): True,
        str(Label("@rules_scala_config//:scala_version")): "3.3.9",
    },
)

def scala_stats_file_off_tests(name):
    """Asserts the Scala 2.13 and Scala 3 toolchains write no stats file."""
    _stats_file(name = name + "_subject")
    _stats_file_off_2_13_test(
        name = name + "_2_13",
        size = "small",
        target_under_test = ":" + name + "_subject",
    )
    _stats_file_off_3_test(
        name = name + "_3",
        size = "small",
        target_under_test = ":" + name + "_subject",
    )

_TestingInfo = provider(
    doc = "Whether a Scala testing toolchain resolved.",
    fields = {"resolved": "True when `ctx.toolchains` has the testing toolchain."},
)

def _testing_impl(ctx):
    return [_TestingInfo(resolved = ctx.toolchains[_TESTING_TOOLCHAIN_TYPE] != None)]

_testing = rule(
    implementation = _testing_impl,
    doc = "Returns whether a Scala testing toolchain resolved.",
    toolchains = [_TESTING_TOOLCHAIN_TYPE],
)

def _testing_resolved_impl(ctx):
    env = analysistest.begin(ctx)
    asserts.true(env, analysistest.target_under_test(env)[_TestingInfo].resolved, "no Scala testing toolchain resolved")
    return analysistest.end(env)

_testing_resolved_2_13_test = analysistest.make(_testing_resolved_impl)

_testing_resolved_3_test = analysistest.make(
    _testing_resolved_impl,
    config_settings = {str(Label("@rules_scala_config//:scala_version")): "3.3.9"},
)

def scala_testing_toolchain_tests(name):
    """Asserts a testing toolchain resolves for Scala 2.13 and Scala 3."""
    _testing(name = name + "_subject")
    _testing_resolved_2_13_test(
        name = name + "_2_13",
        size = "small",
        target_under_test = ":" + name + "_subject",
    )
    _testing_resolved_3_test(
        name = name + "_3",
        size = "small",
        target_under_test = ":" + name + "_subject",
    )
