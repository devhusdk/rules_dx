"""Go documentation shard aspect."""

load("//tools/docs/go:defs.bzl", "go_shard_action")
load("//tools/docs/ir:shard.bzl", "DocsShardInfo")
load("@rules_go//go:def.bzl", "GoInfo")

def _go_docs_shard(target, ctx):
    if GoInfo not in target or DocsShardInfo in target:
        return None
    if ctx.rule.kind.endswith("_test") or ctx.rule.kind.endswith("_binary"):
        return None
    info = target[GoInfo]
    if not info.importpath:
        return None
    return go_shard_action(ctx, info, "", "_dx_docs/" + ctx.label.name, ctx.executable._driver, ctx.executable._go_extract).shard

def _real_go_docs_aspect_impl(target, ctx):
    shard = _go_docs_shard(target, ctx)
    if not shard:
        return []
    return [OutputGroupInfo(dx_results = depset([shard]), dx_docs_shards = depset([shard]))]

real_go_docs_aspect = aspect(
    implementation = _real_go_docs_aspect_impl,
    doc = "Puts the Go docs shard of a target in dx_results and dx_docs_shards.",
    attrs = {
        "_driver": attr.label(
            cfg = "exec",
            default = Label("//tools/docs/go/godoc:godoc_json"),
            executable = True,
        ),
        "_go_extract": attr.label(
            cfg = "exec",
            default = Label("//tools/docs/go/extract:go_extract"),
            executable = True,
        ),
    },
    fragments = ["cpp"],
)
