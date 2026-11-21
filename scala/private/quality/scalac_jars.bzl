"""Exposes the Scala compiler and library jars to a test."""

load("@rules_java//java/common:java_info.bzl", "JavaInfo")
load(":scalac_check.bzl", "SCALA_TOOLCHAIN_TYPE", "scalac_toolchain_jars")

def _scalac_toolchain_jars_impl(ctx):
    jars = scalac_toolchain_jars(ctx.toolchains[SCALA_TOOLCHAIN_TYPE])
    if jars == None:
        fail("the registered rules_scala toolchain is not a Scala 2.13 toolchain")
    return [DefaultInfo(files = depset(transitive = [jars.compiler, jars.library]))]

scalac_toolchain_jars_rule = rule(
    implementation = _scalac_toolchain_jars_impl,
    doc = "The compiler and library jars the scalac driver loads, read the way the aspect reads them.",
    toolchains = [config_common.toolchain_type(SCALA_TOOLCHAIN_TYPE)],
)

def _runtime_jars_impl(ctx):
    return [DefaultInfo(files = depset(transitive = [d[JavaInfo].transitive_runtime_jars for d in ctx.attr.deps]))]

runtime_jars_rule = rule(
    implementation = _runtime_jars_impl,
    doc = "The transitive runtime jars of deps, for a test that loads a compiler the toolchain does not register.",
    attrs = {"deps": attr.label_list(providers = [JavaInfo])},
)
