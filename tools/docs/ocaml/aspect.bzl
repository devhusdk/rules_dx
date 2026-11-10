"""OCaml documentation shard aspect."""

load("//ocaml/private:providers.bzl", _OCamlCmtInfo = "OCamlCmtInfo")
load("//tools/docs/ir:shard.bzl", "DocsShardInfo")
load("//tools/docs/ocaml:defs.bzl", "ODOC_CONFIG", "ocaml_odoc_available", "ocaml_shard_action", "ocaml_typed_trees")

def _ocaml_docs_shard(target, ctx):
    if _OCamlCmtInfo not in target or DocsShardInfo in target:
        return None
    if ctx.rule.kind.endswith("_test") or ctx.rule.kind.endswith("_binary"):
        return None
    if not ocaml_odoc_available(ctx.attr._odoc):
        return None
    trees = ocaml_typed_trees(target[_OCamlCmtInfo])
    if not trees:
        return None
    return ocaml_shard_action(ctx, trees, ctx.label.name, ODOC_CONFIG, "_dx_docs/" + ctx.label.name, ctx.executable._odoc, ctx.executable._ocaml_extract).shard

def _real_ocaml_docs_aspect_impl(target, ctx):
    shard = _ocaml_docs_shard(target, ctx)
    if not shard:
        return []
    return [OutputGroupInfo(dx_results = depset([shard]), dx_docs_shards = depset([shard]))]

real_ocaml_docs_aspect = aspect(
    implementation = _real_ocaml_docs_aspect_impl,
    doc = "Puts the OCaml docs shard of a target in dx_results and dx_docs_shards.",
    attrs = {
        "_ocaml_extract": attr.label(
            cfg = "exec",
            default = Label("//tools/docs/ocaml/extract:ocaml_extract"),
            executable = True,
        ),
        "_odoc": attr.label(
            cfg = "exec",
            default = Label("//tools/docs/ocaml:odoc_tool"),
            executable = True,
        ),
    },
    fragments = ["cpp"],
)
