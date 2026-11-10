"""Fortran documentation shard aspect."""

load("//tools/docs/fortran:defs.bzl", "FORTRAN_SOURCE_EXTENSIONS", "fortran_shard_action")
load("//tools/docs/ir:shard.bzl", "DocsShardInfo")
load("@rules_fortran//fortran:defs.bzl", _FortranInfo = "FortranInfo")

def _fortran_docs_shard(target, ctx):
    if _FortranInfo not in target or DocsShardInfo in target:
        return None
    if ctx.rule.kind.endswith("_test") or ctx.rule.kind.endswith("_binary"):
        return None
    own = [f for f in getattr(ctx.rule.files, "srcs", []) if f.extension in FORTRAN_SOURCE_EXTENSIONS]
    if not own:
        return None
    return fortran_shard_action(ctx, own, ctx.label.name, "_dx_docs/" + ctx.label.name, ctx.executable._ford_driver, ctx.executable._fortran_extract).shard

def _real_fortran_docs_aspect_impl(target, ctx):
    shard = _fortran_docs_shard(target, ctx)
    if not shard:
        return []
    return [OutputGroupInfo(dx_results = depset([shard]), dx_docs_shards = depset([shard]))]

real_fortran_docs_aspect = aspect(
    implementation = _real_fortran_docs_aspect_impl,
    doc = "Puts the Fortran docs shard of a target in dx_results and dx_docs_shards.",
    attrs = {
        "_ford_driver": attr.label(
            cfg = "exec",
            default = Label("//quality/tools/python:ford_dump"),
            executable = True,
        ),
        "_fortran_extract": attr.label(
            cfg = "exec",
            default = Label("//tools/docs/fortran/extract:fortran_extract"),
            executable = True,
        ),
    },
    fragments = ["cpp"],
)
