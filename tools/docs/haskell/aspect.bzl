"""Haskell documentation shard aspect."""

load("//tools/docs/haskell:defs.bzl", "HASKELL_TOOLCHAIN", "haskell_shard_action")
load("//tools/docs/ir:shard.bzl", "DocsShardInfo")
load("@rules_haskell//haskell:providers.bzl", _HaskellInfo = "HaskellInfo")

def _haskell_docs_shard(target, ctx):
    if _HaskellInfo not in target or DocsShardInfo in target:
        return None
    if ctx.rule.kind.endswith("_test") or ctx.rule.kind.endswith("_binary") or ctx.label.name.endswith("_upstream"):
        return None
    if ctx.toolchains[HASKELL_TOOLCHAIN] == None or not target[_HaskellInfo].source_files.to_list():
        return None
    return haskell_shard_action(ctx, target[_HaskellInfo], ctx.label.name, "", "_dx_docs/" + ctx.label.name, ctx.executable._haskell_extract).shard

def _real_haskell_docs_aspect_impl(target, ctx):
    shard = _haskell_docs_shard(target, ctx)
    if not shard:
        return []
    return [OutputGroupInfo(dx_results = depset([shard]), dx_docs_shards = depset([shard]))]

real_haskell_docs_aspect = aspect(
    implementation = _real_haskell_docs_aspect_impl,
    doc = "Puts the Haskell docs shard of a target in dx_results and dx_docs_shards.",
    attrs = {
        "_haskell_extract": attr.label(
            cfg = "exec",
            default = Label("//tools/docs/haskell/extract:haskell_extract"),
            executable = True,
        ),
    },
    fragments = ["cpp"],
    toolchains = [
        config_common.toolchain_type(HASKELL_TOOLCHAIN, mandatory = False),
    ],
)
