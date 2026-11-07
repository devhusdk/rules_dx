"""JMH benchmarks that report to dx bench."""

load("@rules_java//java:defs.bzl", _java_library = "java_library", _java_plugin = "java_plugin", _java_test = "java_test")
load("//java/rules:defs.bzl", "java_library")

_JMH = "@maven//:org_openjdk_jmh_jmh_core"
_JMH_PROCESSOR = "@maven//:org_openjdk_jmh_jmh_generator_annprocess"
_MAIN = "dx.bench.JmhRunner"

def jmh_benchmark(name, library, srcs, deps, jmh, jmh_processor, tags, kwargs, library_kwargs = {}):
    """Declares the JMH library, the runner and the dx-bench test. library is the java_library or kotlin_library macro."""
    processor = name + "_jmh_processor"
    _java_plugin(
        name = processor,
        processor_class = "org.openjdk.jmh.generators.BenchmarkProcessor",
        deps = [jmh_processor],
        testonly = True,
        visibility = ["//visibility:private"],
    )
    library(
        name = name + "_lib",
        srcs = srcs,
        deps = deps + [jmh],
        plugins = [processor],
        testonly = True,
        visibility = ["//visibility:private"],
        warnings_as_errors = False,
        **library_kwargs
    )
    _java_library(
        name = name + "_runner",
        srcs = [Label("//java/rules/jmh:JmhRunner.java")],
        deps = [jmh],
        testonly = True,
        visibility = ["//visibility:private"],
    )
    _java_test(
        name = name,
        main_class = _MAIN,
        use_testrunner = False,
        runtime_deps = [name + "_lib", name + "_runner"],
        tags = tags + [t for t in ["dx-bench", "manual"] if t not in tags],
        **kwargs
    )

def java_benchmark(name, srcs, deps = [], jmh = _JMH, jmh_processor = _JMH_PROCESSOR, tags = [], **kwargs):
    """Runs the JMH benchmarks in srcs and writes dx-bench.json for dx bench.

    Args:
      name: the test target.
      srcs: Java sources with @Benchmark methods.
      deps: libraries the benchmarks use.
      jmh: the jmh-core target.
      jmh_processor: the jmh-generator-annprocess target.
      tags: extra tags. dx-bench and manual are added.
      **kwargs: passed to the test.
    """
    jmh_benchmark(name, java_library, srcs, deps, jmh, jmh_processor, tags, kwargs)
