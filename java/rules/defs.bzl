"""Minimal Java wrappers."""

load("@rules_java//java:defs.bzl", _java_binary = "java_binary", _java_library = "java_library", _java_test = "java_test")
load("@rules_java//java/common:java_info.bzl", "JavaInfo")
load("//java/rules:coverage.bzl", "DX_JVM_TEST_RUNNER", "dx_jvm_test_coverage")
load("//java/rules:jmh.bzl", "jmh_bench")
load("//libs/starlark:fuzz.bzl", "dx_fuzz_runner")
load("//libs/starlark:fuzz_bench.bzl", "merge_tags", "split_runner_kwargs")
load("//libs/starlark:wrapper.bzl", "DX_EXEC_PROVIDES", "dx_executable_forward_rule", "dx_lcov_merger_attr", "dx_library_forward_rule", "dx_without_warnings_attr", "dx_wrap", "dx_wrap_binary", "dx_wrap_test")
load("//quality:sources.bzl", "QualitySourcesInfo")
load(":kwargs.bzl", "java_entry_class", "java_entry_error", "java_entry_kwargs", "java_javacopts_with_warnings_as_errors")

_JMH_MAIN = "dx.bench.JmhRunner"

_DX_JAVA_LIBRARY_PROVIDES = [
    JavaInfo,
    DefaultInfo,
    InstrumentedFilesInfo,
    QualitySourcesInfo,
]

_DX_JAVA_SOURCE_SPECS = [("java", "java")]

_java_library_forward = dx_library_forward_rule(
    provides = _DX_JAVA_LIBRARY_PROVIDES,
    required_providers = [(JavaInfo, "JavaInfo")],
    quality_specs = _DX_JAVA_SOURCE_SPECS,
    what = "java_*",
    allow_files = True,
    upstream_providers = [[JavaInfo]],
)

_java_binary_forward = dx_executable_forward_rule(
    kind = "executable",
    provides = DX_EXEC_PROVIDES,
    required_providers = [],
    quality_specs = _DX_JAVA_SOURCE_SPECS,
    what = "java_*",
    allow_files = True,
    upstream_providers = [[JavaInfo]],
    optional_providers = [JavaInfo],
    runtime = "besteffort",
)

_java_forward_test = dx_executable_forward_rule(
    kind = "test",
    provides = DX_EXEC_PROVIDES,
    required_providers = [],
    quality_specs = _DX_JAVA_SOURCE_SPECS,
    what = "java_*",
    allow_files = True,
    upstream_providers = [[JavaInfo]],
    extra_attrs = dict(
        dx_lcov_merger_attr(),
        main_class = attr.string(default = DX_JVM_TEST_RUNNER),
    ),
    optional_providers = [JavaInfo],
    coverage_providers = dx_jvm_test_coverage,
)

def _java_entry_kwargs_or_fail(name, srcs, kwargs, attr):
    package = native.package_name()
    error = java_entry_error(name, srcs, package, kwargs, attr)
    if error:
        fail(error)
    return java_entry_kwargs(name, srcs, package, kwargs, attr)

def java_library(name, srcs = None, visibility = None, **kwargs):
    """Minimal wrapper over java_library."""
    upstream_kwargs = java_javacopts_with_warnings_as_errors(kwargs)
    dx_wrap(name, _java_library, _java_library_forward, srcs, visibility = visibility, upstream_kwargs = upstream_kwargs, **dx_without_warnings_attr(kwargs))

def java_binary(name, srcs = None, visibility = None, **kwargs):
    """Minimal wrapper over java_binary."""
    upstream_kwargs = _java_entry_kwargs_or_fail(name, srcs, java_javacopts_with_warnings_as_errors(kwargs), "main_class")
    dx_wrap_binary(name, _java_binary, _java_binary_forward, srcs, visibility = visibility, upstream_kwargs = upstream_kwargs, **dx_without_warnings_attr(kwargs))

def java_test(name, srcs = None, visibility = None, **kwargs):
    """Minimal wrapper over java_test."""
    upstream_kwargs = _java_entry_kwargs_or_fail(name, srcs, java_javacopts_with_warnings_as_errors(kwargs), "test_class")
    main_class = kwargs.get("main_class") if kwargs.get("use_testrunner") == False else DX_JVM_TEST_RUNNER
    dx_wrap_test(
        name,
        _java_test,
        _java_forward_test,
        srcs,
        visibility = visibility,
        upstream_kwargs = upstream_kwargs,
        extra_forward_kwargs = {"main_class": main_class},
        **dx_without_warnings_attr(kwargs)
    )

def java_jmh_test(name, runtime_deps, **kwargs):
    """The test target that runs the JMH runner, wrapped like java_test."""
    dx_wrap_test(
        name,
        _java_test,
        _java_forward_test,
        [],
        upstream_kwargs = dict(kwargs, main_class = _JMH_MAIN, use_testrunner = False, runtime_deps = runtime_deps),
        extra_forward_kwargs = {"main_class": _JMH_MAIN},
        quality_srcs = [],
        **kwargs
    )

def java_fuzz_test(name, srcs = None, target_class = None, visibility = None, tags = None, deps = None, **kwargs):
    """A Jazzer fuzz target plus a `<name>_run` runner for `dx fuzz`."""
    target_class = target_class or java_entry_class(name, srcs, native.package_name())
    if target_class == None:
        fail("java_fuzz_test %s needs target_class" % name)
    build, run = split_runner_kwargs(kwargs)
    java_library(
        name = name + "_bin",
        srcs = srcs,
        deps = deps,
        testonly = True,
        tags = merge_tags(tags, "manual"),
        visibility = ["//visibility:private"],
        **build
    )
    _java_binary(
        name = name,
        main_class = "com.code_intelligence.jazzer.Jazzer",
        runtime_deps = [":" + name + "_bin", Label("@dx_jazzer//jar"), Label("@dx_jazzer_api//jar")],
        testonly = True,
        tags = merge_tags(tags, "dx-fuzz", "manual"),
        visibility = visibility,
        **run
    )
    dx_fuzz_runner(name, ":" + name, visibility = visibility, engine = "jazzer", target = target_class)

def java_bench(name, srcs, deps = [], tags = [], **kwargs):
    """Runs the JMH benchmarks in srcs and writes dx-bench.json for dx bench.

    Args:
      name: the test target.
      srcs: Java sources with @Benchmark methods.
      deps: libraries the benchmarks use.
      tags: extra tags. dx-bench and manual are added.
      **kwargs: passed to the test.
    """
    jmh_bench(name, java_library, java_jmh_test, srcs, deps, tags, kwargs)
