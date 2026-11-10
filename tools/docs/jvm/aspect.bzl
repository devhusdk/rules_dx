"""Java, Kotlin and Scala documentation shard aspect."""

load("//tools/docs/ir:shard.bzl", "DocsShardInfo")
load("//tools/docs/jvm:shard.bzl", "JAVA_TOOLCHAIN", "JVM_LANGUAGES", "KOTLIN_TOOLCHAIN", "jvm_shard_action")
load("@rules_java//java/common:java_info.bzl", "JavaInfo")

def _jvm_docs_shard(target, ctx):
    if JavaInfo not in target or DocsShardInfo in target:
        return None
    if ctx.rule.kind.endswith("_test") or ctx.rule.kind.endswith("_binary") or ctx.label.name.endswith("_upstream"):
        return None
    srcs = sorted(getattr(ctx.rule.files, "srcs", []), key = lambda f: f.path)
    if not srcs or getattr(ctx.rule.attr, "plugins", []):
        return None
    for language, spec in JVM_LANGUAGES.items():
        if all([f.extension == spec.extension for f in srcs]):
            if ctx.toolchains[JAVA_TOOLCHAIN] == None or (language == "kotlin" and ctx.toolchains[KOTLIN_TOOLCHAIN] == None):
                return None
            adapter = getattr(ctx.executable, "_" + language + "_extract")
            action = jvm_shard_action(ctx, language, target[JavaInfo], srcs, ctx.label.name, [], "_dx_docs/" + ctx.label.name, adapter, ctx.executable._jvm_convert)
            return action.shard
    return None

def _real_jvm_docs_aspect_impl(target, ctx):
    shard = _jvm_docs_shard(target, ctx)
    if not shard:
        return []
    return [OutputGroupInfo(dx_results = depset([shard]), dx_docs_shards = depset([shard]))]

real_jvm_docs_aspect = aspect(
    implementation = _real_jvm_docs_aspect_impl,
    doc = "Puts the Java, Kotlin and Scala docs shard of a target in dx_results and dx_docs_shards.",
    attrs = {
        "_java_extract": attr.label(
            cfg = "exec",
            default = Label("//tools/docs/jvm/javadoc:javadoc_extract"),
            executable = True,
        ),
        "_jvm_convert": attr.label(
            cfg = "exec",
            default = Label("//tools/docs/jvm/convert:jvm_convert"),
            executable = True,
        ),
        "_kotlin_extract": attr.label(
            cfg = "exec",
            default = Label("//tools/docs/jvm/dokka:dokka_extract"),
            executable = True,
        ),
        "_scala_extract": attr.label(
            cfg = "exec",
            default = Label("//tools/docs/jvm/scaladoc:scaladoc_extract"),
            executable = True,
        ),
    },
    fragments = ["cpp"],
    toolchains = [
        config_common.toolchain_type(JAVA_TOOLCHAIN, mandatory = False),
        config_common.toolchain_type(KOTLIN_TOOLCHAIN, mandatory = False),
    ],
)
