"""Zig documentation shard aspect."""

load("//tools/docs/ir:shard.bzl", "DocsShardInfo")
load("//tools/docs/zig:defs.bzl", "zig_shard_action")
load("@rules_zig//zig/private/providers:zig_module_info.bzl", _ZigModuleInfo = "ZigModuleInfo")

def _zig_docs_shard(target, ctx):
    if _ZigModuleInfo not in target or DocsShardInfo in target:
        return None
    if ctx.rule.kind.endswith("_test") or ctx.rule.kind.endswith("_binary") or ctx.label.name.endswith("_upstream"):
        return None
    own = [f for f in getattr(ctx.rule.files, "srcs", []) if f.extension == "zig"]
    if not own:
        return None
    return zig_shard_action(ctx, own, ctx.label.name, "", "_dx_docs/" + ctx.label.name, ctx.executable._zig_dump, ctx.executable._zig_extract).shard

def _real_zig_docs_aspect_impl(target, ctx):
    shard = _zig_docs_shard(target, ctx)
    if not shard:
        return []
    return [OutputGroupInfo(dx_results = depset([shard]), dx_docs_shards = depset([shard]))]

real_zig_docs_aspect = aspect(
    implementation = _real_zig_docs_aspect_impl,
    doc = "Puts the Zig docs shard of a target in dx_results and dx_docs_shards.",
    attrs = {
        "_zig_dump": attr.label(
            cfg = "exec",
            default = Label("//tools/docs/zig/dump:zig_doc_dump"),
            executable = True,
        ),
        "_zig_extract": attr.label(
            cfg = "exec",
            default = Label("//tools/docs/zig/extract:zig_extract"),
            executable = True,
        ),
    },
    fragments = ["cpp"],
)
