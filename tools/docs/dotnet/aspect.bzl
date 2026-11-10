"""C# and F# documentation shard aspect."""

load("//tools/docs/dotnet:shard.bzl", "DotnetDocsInfo", "dotnet_docs_collect_aspect", "dotnet_docs_problem", "dotnet_shard_action")
load("//tools/docs/ir:shard.bzl", "DocsShardInfo")

def _dotnet_docs_shard(target, ctx):
    if DocsShardInfo in target or DotnetDocsInfo not in target:
        return None
    if ctx.rule.kind.endswith("_test") or ctx.rule.kind.endswith("_binary"):
        return None
    info = target[DotnetDocsInfo]
    if dotnet_docs_problem(info, info.language):
        return None
    driver = ctx.executable._csharp_driver if info.language == "csharp" else ctx.executable._fsharp_driver
    return dotnet_shard_action(ctx, info, info.assembly, "_dx_docs/" + ctx.label.name, driver, ctx.executable._dotnet_extract).shard

def _real_dotnet_docs_aspect_impl(target, ctx):
    shard = _dotnet_docs_shard(target, ctx)
    if not shard:
        return []
    return [OutputGroupInfo(dx_results = depset([shard]), dx_docs_shards = depset([shard]))]

real_dotnet_docs_aspect = aspect(
    implementation = _real_dotnet_docs_aspect_impl,
    doc = "Puts the C# and F# docs shard of a target in dx_results and dx_docs_shards.",
    attrs = {
        "_csharp_driver": attr.label(
            cfg = "exec",
            default = Label("//tools/docs/dotnet/csharp:csharp_api_pinned"),
            executable = True,
        ),
        "_dotnet_extract": attr.label(
            cfg = "exec",
            default = Label("//tools/docs/dotnet/extract:dotnet_extract"),
            executable = True,
        ),
        "_fsharp_driver": attr.label(
            cfg = "exec",
            default = Label("//tools/docs/dotnet/fsharp:fsharp_api_pinned"),
            executable = True,
        ),
    },
    fragments = ["cpp"],
    requires = [dotnet_docs_collect_aspect],
)
