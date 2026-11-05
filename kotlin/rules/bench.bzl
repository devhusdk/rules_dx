"""Kotlin JMH benchmarks that report to dx bench."""

load("@rules_kotlin//kotlin:core.bzl", "kt_javac_options")
load("//java/rules:bench.bzl", "jmh_benchmark")
load("//kotlin/rules:defs.bzl", "kotlin_library")

def kotlin_benchmark(name, srcs, deps = [], jmh = "@maven//:org_openjdk_jmh_jmh_core", jmh_processor = "@maven//:org_openjdk_jmh_jmh_generator_annprocess", tags = [], **kwargs):
    """Runs the JMH benchmarks in Kotlin srcs and writes dx-bench.json for dx bench.

    Args:
      name: the test target.
      srcs: Kotlin sources with @Benchmark methods.
      deps: libraries the benchmarks use.
      jmh: the jmh-core target.
      jmh_processor: the jmh-generator-annprocess target.
      tags: extra tags. dx-bench and manual are added.
      **kwargs: passed to the test.
    """
    kt_javac_options(
        name = name + "_javac_opts",
        warn = "off",
        testonly = True,
        visibility = ["//visibility:private"],
    )
    jmh_benchmark(name, kotlin_library, srcs, deps, jmh, jmh_processor, tags, kwargs, {"javac_opts": ":" + name + "_javac_opts"})
