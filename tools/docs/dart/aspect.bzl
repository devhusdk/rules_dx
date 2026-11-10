"""Dart documentation shard aspect."""

load("//tools/docs/dart:defs.bzl", "dart_shard_action")
load("//tools/docs/ir:shard.bzl", "DocsShardInfo", "repo_relative")
load("@rules_dart//dart:providers.bzl", _DartInfo = "DartInfo")

_DART_TOOLCHAIN = "@rules_dart//dart:toolchain_type"

def _dart_docs_shard(target, ctx):
    if _DartInfo not in target or DocsShardInfo in target:
        return None
    if ctx.rule.kind.endswith("_test") or ctx.rule.kind.endswith("_binary") or ctx.label.name.endswith("_upstream"):
        return None
    info = target[_DartInfo]
    root = info.lib_root + "/lib/" if info.lib_root else "lib/"
    if ctx.toolchains[_DART_TOOLCHAIN] == None or not [f for f in info.srcs if repo_relative(f.short_path).startswith(root) and f.extension == "dart"]:
        return None
    return dart_shard_action(ctx, info, info.package_name, "_dx_docs/" + ctx.label.name, ctx.executable._dart_extract).shard

def _real_dart_docs_aspect_impl(target, ctx):
    shard = _dart_docs_shard(target, ctx)
    if not shard:
        return []
    return [OutputGroupInfo(dx_results = depset([shard]), dx_docs_shards = depset([shard]))]

real_dart_docs_aspect = aspect(
    implementation = _real_dart_docs_aspect_impl,
    doc = "Puts the Dart docs shard of a target in dx_results and dx_docs_shards.",
    attrs = {
        "_dart_extract": attr.label(
            cfg = "exec",
            default = Label("//tools/docs/dart/extract:dart_extract"),
            executable = True,
        ),
    },
    fragments = ["cpp"],
    toolchains = [
        config_common.toolchain_type(_DART_TOOLCHAIN, mandatory = False),
    ],
)
