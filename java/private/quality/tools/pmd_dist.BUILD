"""PMD distribution library closure for the JVM wrapper."""

load("@rules_java//java:defs.bzl", "java_import", "java_library")

package(default_visibility = ["//visibility:public"])

java_import(
    name = "import",
    jars = glob(["lib/*.jar"]),
)

java_library(
    name = "lib",
    visibility = ["//visibility:public"],
    runtime_deps = [":import"],
)
