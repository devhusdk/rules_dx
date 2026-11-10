"""Protobuf documentation shard aspect."""

load("//cli/exec_os:defs.bzl", "ExecOsInfo")
load("//tools/docs/ir:shard.bzl", "DocsShardInfo")
load("//tools/docs/protobuf:defs.bzl", "PROTO_TOOLCHAIN", "protobuf_shard_action")
load("@protobuf//bazel/common:proto_info.bzl", _ProtoInfo = "ProtoInfo")

def _protobuf_docs_shard(target, ctx):
    if _ProtoInfo not in target or DocsShardInfo in target:
        return None
    proto_info = target[_ProtoInfo]
    toolchain = ctx.toolchains[PROTO_TOOLCHAIN]
    if not proto_info.direct_sources or toolchain == None:
        return None
    return protobuf_shard_action(ctx, proto_info, toolchain.proto.proto_compiler, ctx.attr._exec_os[ExecOsInfo].windows, ctx.label.name, "_dx_docs/" + ctx.label.name, ctx.executable._protobuf_extract).shard

def _real_protobuf_docs_aspect_impl(target, ctx):
    shard = _protobuf_docs_shard(target, ctx)
    if not shard:
        return []
    return [OutputGroupInfo(dx_results = depset([shard]), dx_docs_shards = depset([shard]))]

real_protobuf_docs_aspect = aspect(
    implementation = _real_protobuf_docs_aspect_impl,
    doc = "Puts the Protobuf docs shard of a target in dx_results and dx_docs_shards.",
    attrs = {
        "_exec_os": attr.label(
            default = Label("//cli/exec_os"),
            cfg = "exec",
            providers = [ExecOsInfo],
        ),
        "_protobuf_extract": attr.label(
            cfg = "exec",
            default = Label("//tools/docs/protobuf/extract:protobuf_extract"),
            executable = True,
        ),
    },
    fragments = ["cpp"],
    toolchains = [
        config_common.toolchain_type(PROTO_TOOLCHAIN, mandatory = False),
    ],
)
