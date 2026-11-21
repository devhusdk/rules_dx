"""Kotlin documentation shard aspect."""

load("//tools/docs/jvm:shard.bzl", "JAVA_TOOLCHAIN", "KOTLIN_TOOLCHAIN", "jvm_docs_aspect_result")

def _real_kotlin_docs_aspect_impl(target, ctx):
    if ctx.toolchains[KOTLIN_TOOLCHAIN] == None:
        return []
    return jvm_docs_aspect_result(target, ctx, "kotlin")

real_kotlin_docs_aspect = aspect(
    implementation = _real_kotlin_docs_aspect_impl,
    doc = "Puts the Kotlin docs shard of a target in dx_results and dx_docs_shards.",
    attrs = {
        "_jvm_convert": attr.label(
            cfg = "exec",
            default = Label("@dx_prebuilt//:dx"),
            executable = True,
        ),
        "_kotlin_extract": attr.label(
            cfg = "exec",
            default = Label("@dx_prebuilt//:dokka_extract"),
            executable = True,
        ),
    },
    fragments = ["cpp"],
    toolchains = [
        config_common.toolchain_type(JAVA_TOOLCHAIN, mandatory = False),
        config_common.toolchain_type(KOTLIN_TOOLCHAIN, mandatory = False),
    ],
)
