"""Minimal Scala wrappers."""

load("@rules_java//java:defs.bzl", "JavaInfo")
load("@rules_scala//scala:scala.bzl", _scala_binary = "scala_binary", _scala_library = "scala_library", _scala_test = "scala_test")
load("@rules_scala//scala:semanticdb_provider.bzl", "SemanticdbInfo")
load("//java/rules:coverage.bzl", "dx_jvm_test_coverage")
load("//java/rules:defs.bzl", "java_jmh_test")
load("//java/rules:jazzer.bzl", "jazzer_fuzz")
load("//java/rules:jmh.bzl", "jmh_bench_generated")
load("//libs/starlark:wrapper.bzl", "DX_EXEC_PROVIDES", "dx_executable_forward_rule", "dx_lcov_merger_attr", "dx_library_forward_rule", "dx_without_warnings_attr", "dx_wrap", "dx_wrap_binary", "dx_wrap_test")
load("//quality:sources.bzl", "QualitySourcesInfo")
load(":validate.bzl", "scala_scalacopts_with_warnings_as_errors")

_SCALA_TEST_RUNNER = "io.bazel.rulesscala.scala_test.Runner"

_DX_SCALA_LIBRARY_PROVIDES = [
    JavaInfo,
    DefaultInfo,
    InstrumentedFilesInfo,
    QualitySourcesInfo,
]

_DX_SCALA_SOURCE_SPECS = [("scala", "scala"), ("java", "java")]

_scala_library_forward = dx_library_forward_rule(
    provides = _DX_SCALA_LIBRARY_PROVIDES,
    required_providers = [(JavaInfo, "JavaInfo")],
    quality_specs = _DX_SCALA_SOURCE_SPECS,
    what = "scala_*",
    allow_files = True,
    upstream_providers = [[JavaInfo]],
    quiet_providers = [SemanticdbInfo],
)

_scala_binary_forward = dx_executable_forward_rule(
    kind = "executable",
    provides = DX_EXEC_PROVIDES,
    required_providers = [],
    quality_specs = _DX_SCALA_SOURCE_SPECS,
    what = "scala_*",
    allow_files = True,
    upstream_providers = [[JavaInfo]],
    optional_providers = [JavaInfo],
    quiet_providers = [SemanticdbInfo],
    runtime = "besteffort",
)

_scala_forward_test = dx_executable_forward_rule(
    kind = "test",
    provides = DX_EXEC_PROVIDES,
    required_providers = [],
    quality_specs = _DX_SCALA_SOURCE_SPECS,
    what = "scala_*",
    allow_files = True,
    upstream_providers = [[JavaInfo]],
    extra_attrs = dict(
        dx_lcov_merger_attr(),
        main_class = attr.string(default = _SCALA_TEST_RUNNER),
    ),
    optional_providers = [JavaInfo],
    quiet_providers = [SemanticdbInfo],
    coverage_providers = dx_jvm_test_coverage,
)

def scala_library(name, srcs = None, visibility = None, **kwargs):
    """Minimal wrapper over scala_library."""
    dx_wrap(name, _scala_library, _scala_library_forward, srcs, visibility = visibility, upstream_kwargs = scala_scalacopts_with_warnings_as_errors(kwargs), **dx_without_warnings_attr(kwargs))

def scala_binary(name, srcs = None, visibility = None, **kwargs):
    """Minimal wrapper over scala_binary. -Xfatal-warnings is on unless warnings_as_errors is False."""
    dx_wrap_binary(name, _scala_binary, _scala_binary_forward, srcs, visibility = visibility, upstream_kwargs = scala_scalacopts_with_warnings_as_errors(kwargs), **dx_without_warnings_attr(kwargs))

def scala_test(name, srcs = None, visibility = None, **kwargs):
    """Minimal wrapper over scala_test. The dx test runner runs unless main_class is set. -Xfatal-warnings is on unless warnings_as_errors is False."""
    dx_wrap_test(
        name,
        _scala_test,
        _scala_forward_test,
        srcs,
        visibility = visibility,
        upstream_kwargs = scala_scalacopts_with_warnings_as_errors(kwargs),
        extra_forward_kwargs = {"main_class": kwargs.get("main_class") or _SCALA_TEST_RUNNER},
        **dx_without_warnings_attr(kwargs)
    )

def scala_fuzz_test(name, srcs, target_class, deps = None, visibility = None, tags = None, **kwargs):
    """A Jazzer fuzz target plus a `<name>_run` runner for `dx fuzz`."""
    jazzer_fuzz(name, scala_library, srcs, deps, target_class, visibility, tags, kwargs)

def scala_bench(name, srcs, deps = [], tags = [], **kwargs):
    """Runs the JMH benchmarks in Scala srcs and writes dx-bench.json for dx bench."""
    jmh_bench_generated(name, scala_library, java_jmh_test, srcs, deps, tags, kwargs)
