"""Avro documentation shard aspect."""

load("//avro/private:providers.bzl", _AvroInfo = "AvroInfo")
load("//tools/docs/avro:defs.bzl", "avro_shard_action")
load("//tools/docs/ir:shard.bzl", "DocsShardInfo")

def _avro_docs_shard(target, ctx):
    if _AvroInfo not in target or DocsShardInfo in target:
        return None
    info = target[_AvroInfo]
    if not info.direct_srcs:
        return None
    return avro_shard_action(ctx, info.direct_srcs, info.transitive_srcs, ctx.attr._avro_tools[DefaultInfo].files_to_run, ctx.label.name, "_dx_docs/" + ctx.label.name, ctx.executable._avro_extract).shard

def _real_avro_docs_aspect_impl(target, ctx):
    shard = _avro_docs_shard(target, ctx)
    if not shard:
        return []
    return [OutputGroupInfo(dx_results = depset([shard]), dx_docs_shards = depset([shard]))]

real_avro_docs_aspect = aspect(
    implementation = _real_avro_docs_aspect_impl,
    doc = "Puts the Avro docs shard of a target in dx_results and dx_docs_shards.",
    attrs = {
        "_avro_extract": attr.label(
            cfg = "exec",
            default = Label("//tools/docs/avro/extract:avro_extract"),
            executable = True,
        ),
        "_avro_tools": attr.label(
            cfg = "exec",
            default = Label("//config:tool_avro"),
            executable = True,
        ),
    },
    fragments = ["cpp"],
)
