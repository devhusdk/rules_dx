"""Exposes the Scala compiler and library jars of the registered rules_scala toolchain to a test."""

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
