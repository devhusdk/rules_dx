"""Julia documentation shard aspect."""

load("//tools/docs/ir:shard.bzl", "DocsShardInfo")
load("//tools/docs/julia:defs.bzl", "julia_shard_action")
load("@rules_julia//julia:julia_info.bzl", _JuliaInfo = "JuliaInfo")

def _julia_docs_shard(target, ctx):
    if _JuliaInfo not in target or DocsShardInfo in target:
        return None
    if ctx.rule.kind.endswith("_test") or ctx.rule.kind.endswith("_binary"):
        return None
    info = target[_JuliaInfo]
    if not [f for f in info.srcs.to_list() if f.extension == "jl"]:
        return None
    return julia_shard_action(ctx, info.srcs.to_list(), info.transitive_srcs.to_list(), ctx.label.name, ctx.label.name, "_dx_docs/" + ctx.label.name, ctx.executable._julia_driver, ctx.executable._julia_extract).shard

def _real_julia_docs_aspect_impl(target, ctx):
    shard = _julia_docs_shard(target, ctx)
    if not shard:
        return []
    return [OutputGroupInfo(dx_results = depset([shard]), dx_docs_shards = depset([shard]))]

real_julia_docs_aspect = aspect(
    implementation = _real_julia_docs_aspect_impl,
    doc = "Puts the Julia docs shard of a target in dx_results and dx_docs_shards.",
    attrs = {
        "_julia_driver": attr.label(
            cfg = "exec",
            default = Label("//tools/docs/julia:doc_dump"),
            executable = True,
        ),
        "_julia_extract": attr.label(
            cfg = "exec",
            default = Label("//tools/docs/julia/extract:julia_extract"),
            executable = True,
        ),
    },
    fragments = ["cpp"],
)
