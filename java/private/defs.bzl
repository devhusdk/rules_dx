"""Minimal Java wrappers."""

load("@rules_java//java:defs.bzl", _java_binary = "java_binary", _java_library = "java_library", _java_test = "java_test")
load("@rules_java//java/common:java_info.bzl", "JavaInfo")
load("//java/private:coverage.bzl", "DX_JVM_TEST_RUNNER", "dx_jvm_test_coverage")
load("//java/private:jazzer.bzl", "jazzer_fuzz", "jazzer_target_class")
load("//java/private:jmh.bzl", "jmh_bench")
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

def java_binary(name, *, srcs = None, visibility = None, **kwargs):
    """Minimal wrapper over java_binary. main_class is inferred from the name and srcs when unset. -Xlint:all is on; warnings are errors unless warnings_as_errors is False."""
    upstream_kwargs = _java_entry_kwargs_or_fail(name, srcs, java_javacopts_with_warnings_as_errors(kwargs), "main_class")
    dx_wrap_binary(name, _java_binary, _java_binary_forward, srcs, visibility = visibility, upstream_kwargs = upstream_kwargs, **dx_without_warnings_attr(kwargs))

def java_test(name, *, srcs = None, visibility = None, **kwargs):
    """Minimal wrapper over java_test. test_class is inferred from the name and srcs when unset; the dx test runner runs unless use_testrunner = False. -Xlint:all is on; warnings are errors unless warnings_as_errors is False."""
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

def java_jmh_test(name, *, runtime_deps, **kwargs):
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

def java_fuzz_test(name, *, srcs = None, target_class = None, deps = None, visibility = None, tags = None, **kwargs):
    """A Jazzer fuzz target plus a `<name>_run` runner for `dx fuzz`. target_class is inferred from the name or the single src when unset."""
    target_class = jazzer_target_class("java_fuzz_test", name, srcs, target_class, native.package_name(), ".java")
    jazzer_fuzz(name, java_library, srcs, deps, target_class, visibility, tags, kwargs)

def java_bench(name, *, srcs, deps = None, tags = None, **kwargs):
    """Runs the JMH benchmarks in srcs and writes dx-bench.json for dx bench."""
    jmh_bench(name, java_library, java_jmh_test, srcs, deps or [], tags or [], kwargs)
