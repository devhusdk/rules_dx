"""Java documentation shard aspect."""

load("//tools/docs/jvm:shard.bzl", "JAVA_TOOLCHAIN", "jvm_docs_aspect_result")

def _real_jvm_docs_aspect_impl(target, ctx):
    return jvm_docs_aspect_result(target, ctx, "java")

real_jvm_docs_aspect = aspect(
    implementation = _real_jvm_docs_aspect_impl,
    doc = "Puts the Java docs shard of a target in dx_results and dx_docs_shards.",
    attrs = {
        "_java_extract": attr.label(
            cfg = "exec",
            default = Label("@dx_prebuilt//:javadoc_extract"),
            executable = True,
        ),
        "_jvm_convert": attr.label(
            cfg = "exec",
            default = Label("@dx_prebuilt//:dx"),
            executable = True,
        ),
    },
    fragments = ["cpp"],
    toolchains = [
        config_common.toolchain_type(JAVA_TOOLCHAIN, mandatory = False),
    ],
)
