"""Scala documentation shard aspect."""

load("//tools/docs/jvm:shard.bzl", "JAVA_TOOLCHAIN", "jvm_docs_aspect_result")
load("//tools/docs/scala:version.bzl", "SCALA_VERSION_ATTRS", "unsupported_scala_version")

def _real_scala_docs_aspect_impl(target, ctx):
    if unsupported_scala_version(ctx):
        return []
    return jvm_docs_aspect_result(target, ctx, "scala")

real_scala_docs_aspect = aspect(
    implementation = _real_scala_docs_aspect_impl,
    doc = "Puts the Scala docs shard of a target in dx_results and dx_docs_shards.",
    attrs = SCALA_VERSION_ATTRS | {
        "_jvm_convert": attr.label(
            cfg = "exec",
            default = Label("@dx_prebuilt//dx:jvm_convert"),
            executable = True,
        ),
        "_scala_extract": attr.label(
            cfg = "exec",
            default = Label("@dx_prebuilt//:scaladoc_extract"),
            executable = True,
        ),
    },
    fragments = ["cpp"],
    toolchains = [
        config_common.toolchain_type(JAVA_TOOLCHAIN, mandatory = False),
    ],
)
