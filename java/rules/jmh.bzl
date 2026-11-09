"""Private JMH helper for java_bench and kotlin_bench."""

load("@rules_java//java:defs.bzl", _java_library = "java_library", _java_plugin = "java_plugin", _java_test = "java_test")
load("//libs/starlark:fuzz_bench.bzl", "DEFAULT_SIZE", "merge_tags")

_MAIN = "dx.bench.JmhRunner"
_JMH = [Label("@dx_jmh_core//jar"), Label("@dx_jmh_jopt_simple//jar"), Label("@dx_jmh_commons_math3//jar")]
_PROCESSOR = [Label("@dx_jmh_generator_annprocess//jar")] + _JMH

def jmh_bench(name, library, srcs, deps, tags, kwargs, library_kwargs = {}):
    """Declares the JMH library, the runner and the dx-bench test. library is the java_library or kotlin_library macro."""
    processor = name + "_jmh_processor"
    _java_plugin(
        name = processor,
        processor_class = "org.openjdk.jmh.generators.BenchmarkProcessor",
        deps = _PROCESSOR,
        testonly = True,
        visibility = ["//visibility:private"],
    )
    library(
        name = name + "_bin",
        srcs = srcs,
        deps = deps + _JMH,
        plugins = [processor],
        testonly = True,
        visibility = ["//visibility:private"],
        warnings_as_errors = False,
        **library_kwargs
    )
    _java_library(
        name = name + "_jmh_runner",
        srcs = [Label("//java/rules/jmh:JmhRunner.java")],
        deps = _JMH,
        testonly = True,
        visibility = ["//visibility:private"],
    )
    _java_test(
        name = name,
        main_class = _MAIN,
        use_testrunner = False,
        runtime_deps = [name + "_bin", name + "_jmh_runner"],
        tags = merge_tags(tags, "dx-bench", "manual"),
        testonly = True,
        **dict(kwargs, size = kwargs.get("size", DEFAULT_SIZE))
    )
