"""Swift documentation shard aspect."""

load("//swift/rules:providers.bzl", "SwiftDocsInfo")
load("//tools/docs/ir:shard.bzl", "DocsShardInfo")
load("//tools/docs/swift:defs.bzl", "swift_shard_action")

def _swift_docs_shard(target, ctx):
    if SwiftDocsInfo not in target or DocsShardInfo in target or ctx.label.name.endswith("_upstream"):
        return None
    info = target[SwiftDocsInfo]
    if len(info.graphs) != 1 or not info.srcs:
        return None
    return swift_shard_action(ctx, info, ctx.label.name, "", "_dx_docs/" + ctx.label.name, ctx.executable._swift_extract).shard

def _real_swift_docs_aspect_impl(target, ctx):
    shard = _swift_docs_shard(target, ctx)
    if not shard:
        return []
    return [OutputGroupInfo(dx_results = depset([shard]), dx_docs_shards = depset([shard]))]

real_swift_docs_aspect = aspect(
    implementation = _real_swift_docs_aspect_impl,
    doc = "Puts the Swift docs shard of a target in dx_results and dx_docs_shards.",
    attrs = {
        "_swift_extract": attr.label(
            cfg = "exec",
            default = Label("//tools/docs/swift/extract:swift_extract"),
            executable = True,
        ),
    },
    fragments = ["cpp"],
)
