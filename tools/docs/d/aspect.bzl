"""D documentation shard aspect."""

load("//tools/docs/d:defs.bzl", "D_TOOLCHAIN", "d_shard_action")
load("//tools/docs/ir:shard.bzl", "DocsShardInfo")
load("@rules_d//d/private:providers.bzl", _DInfo = "DInfo")

def _d_docs_shard(target, ctx):
    if _DInfo not in target or DocsShardInfo in target:
        return None
    if ctx.rule.kind.endswith("_test") or ctx.rule.kind.endswith("_binary") or ctx.label.name.endswith("_upstream"):
        return None
    own = [f for f in getattr(ctx.rule.files, "srcs", []) if f.extension in ("d", "di")]
    if not own or ctx.toolchains[D_TOOLCHAIN] == None:
        return None
    return d_shard_action(ctx, target[_DInfo], own, ctx.label.name, "", "_dx_docs/" + ctx.label.name, ctx.executable._d_extract).shard

def _real_d_docs_aspect_impl(target, ctx):
    shard = _d_docs_shard(target, ctx)
    if not shard:
        return []
    return [OutputGroupInfo(dx_results = depset([shard]), dx_docs_shards = depset([shard]))]

real_d_docs_aspect = aspect(
    implementation = _real_d_docs_aspect_impl,
    doc = "Puts the D docs shard of a target in dx_results and dx_docs_shards.",
    attrs = {
        "_d_extract": attr.label(
            cfg = "exec",
            default = Label("//tools/docs/d/extract:d_extract"),
            executable = True,
        ),
    },
    fragments = ["cpp"],
    toolchains = [
        config_common.toolchain_type(D_TOOLCHAIN, mandatory = False),
    ],
)
