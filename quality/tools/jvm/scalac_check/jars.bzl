"""Exposes the Scala compiler and library jars of the registered rules_scala toolchain to a test."""

load("@rules_java//java/common:java_info.bzl", "JavaInfo")
load("//quality:jvm_check.bzl", "scalac_toolchain_jars")

_TOOLCHAIN_TYPE = "@rules_scala//scala:toolchain_type"

def _scalac_toolchain_jars_impl(ctx):
    jars = scalac_toolchain_jars(ctx.toolchains[_TOOLCHAIN_TYPE])
    if jars == None:
        fail("the registered rules_scala toolchain is not a Scala 2.13 toolchain")
    return [DefaultInfo(files = depset(transitive = [jars.compiler, jars.library]))]

scalac_toolchain_jars_rule = rule(
    implementation = _scalac_toolchain_jars_impl,
    doc = "The compiler and library jars the scalac driver loads, read the way the aspect reads them.",
    toolchains = [config_common.toolchain_type(_TOOLCHAIN_TYPE)],
)

def _runtime_jars_impl(ctx):
    return [DefaultInfo(files = depset(transitive = [d[JavaInfo].transitive_runtime_jars for d in ctx.attr.deps]))]

runtime_jars_rule = rule(
    implementation = _runtime_jars_impl,
    doc = "The transitive runtime jars of deps, for a test that loads a compiler the toolchain does not register.",
    attrs = {"deps": attr.label_list(providers = [JavaInfo])},
)
