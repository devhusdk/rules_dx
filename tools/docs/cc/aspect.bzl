"""C, C++ and CUDA documentation shard aspect."""

load("//tools/docs/cc:defs.bzl", "cc_shard_action", "doxygen_available")
load("//tools/docs/ir:shard.bzl", "DocsShardInfo")
load("@rules_cc//cc/common:cc_info.bzl", "CcInfo")

_CC_DOCS_TOOLCHAIN = "@bazel_tools//tools/cpp:toolchain_type"

def _cc_docs_shard(target, ctx):
    if CcInfo not in target or DocsShardInfo in target or ctx.label.name.endswith("_upstream"):
        return None
    language = "cuda" if "cuda_library" in ctx.rule.kind else ("cc" if "cc_library" in ctx.rule.kind else None)
    if not language:
        return None
    headers = target[CcInfo].compilation_context.direct_public_headers
    doxygen = ctx.attr._doxygen[DefaultInfo].files.to_list()
    if not headers or not doxygen_available(doxygen):
        return None
    return cc_shard_action(ctx, language, headers, ctx.label.name, "_dx_docs/" + ctx.label.name, doxygen, ctx.executable._cc_extract).shard

def _real_cc_docs_aspect_impl(target, ctx):
    shard = _cc_docs_shard(target, ctx)
    if not shard:
        return []
    return [OutputGroupInfo(dx_results = depset([shard]), dx_docs_shards = depset([shard]))]

real_cc_docs_aspect = aspect(
    implementation = _real_cc_docs_aspect_impl,
    doc = "Puts the C, C++ and CUDA docs shard of a target in dx_results and dx_docs_shards.",
    attrs = {
        "_cc_extract": attr.label(
            cfg = "exec",
            default = Label("//tools/docs/cc/extract:cc_extract"),
            executable = True,
        ),
        "_doxygen": attr.label(
            cfg = "exec",
            default = Label("@dx_doxygen//:tool"),
        ),
    },
    fragments = ["cpp"],
    toolchains = [
        config_common.toolchain_type(_CC_DOCS_TOOLCHAIN, mandatory = False),
    ],
)
