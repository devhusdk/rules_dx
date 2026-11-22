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

_THIRD_VERSION = "2.12.21"

_ScalaVersionInfo = provider(
    doc = "The resolved Scala toolchain's version, whether it is the dx semanticdb one, and whether a testing toolchain resolved.",
    fields = {
        "dx": "True when the Scala toolchain is rules_dx's semanticdb toolchain.",
        "testing": "True when a testing toolchain resolved.",
        "version": "The Scala toolchain's `scala_version`.",
    },
)

def _scala_version_impl(ctx):
    toolchain = ctx.toolchains[_SCALA_TOOLCHAIN_TYPE]
    return [_ScalaVersionInfo(
        dx = "-P:semanticdb:text:on" in toolchain.scalacopts,
        testing = ctx.toolchains[_TESTING_TOOLCHAIN_TYPE] != None,
        version = toolchain.scala_version,
    )]

_scala_version = rule(
    implementation = _scala_version_impl,
    doc = "Returns the resolved Scala toolchain's version, whether it is the dx semanticdb one, and whether a testing toolchain resolved.",
    toolchains = [_SCALA_TOOLCHAIN_TYPE, _TESTING_TOOLCHAIN_TYPE],
)

def _third_version_impl(ctx):
    env = analysistest.begin(ctx)
    info = analysistest.target_under_test(env)[_ScalaVersionInfo]
    asserts.equals(env, _THIRD_VERSION, info.version)
    asserts.true(env, info.dx, "the dx semanticdb toolchain did not resolve")
    asserts.true(env, info.testing, "no Scala testing toolchain resolved")
    return analysistest.end(env)

_third_version_test = analysistest.make(
    _third_version_impl,
    config_settings = {
        str(Label("//config:toolchains")): True,
        str(Label("@rules_scala_config//:scala_version")): _THIRD_VERSION,
    },
)

def scala_third_version_tests(name, size):
    """Asserts the dx and testing toolchains resolve for a Scala version the root module adds with `scala_config.settings`."""
    _scala_version(name = name + "_subject")
    _third_version_test(
        name = name,
        size = size,
        target_under_test = ":" + name + "_subject",
    )
