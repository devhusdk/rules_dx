"""Rust documentation shard aspect."""

load("//tools/docs/ir:shard.bzl", "DocsShardInfo")
load("//tools/docs/rust:defs.bzl", "rust_doc_inputs_aspect", "rust_shard_action")
load("@rules_rust//rust:defs.bzl", _rust_common = "rust_common")

_RUST_DOCS_TOOLCHAIN = str(Label("@rules_rust//rust:toolchain_type"))
_CC_DOCS_TOOLCHAIN = "@bazel_tools//tools/cpp:toolchain_type"

def _rust_docs_shard(target, ctx):
    if _rust_common.crate_info not in target or DocsShardInfo in target:
        return None
    crate = target[_rust_common.crate_info]
    if crate.type not in ("rlib", "lib") or crate.is_test:
        return None
    if not ctx.toolchains[_RUST_DOCS_TOOLCHAIN] or not ctx.toolchains[_CC_DOCS_TOOLCHAIN]:
        return None
    return rust_shard_action(ctx, target, "", stem = "_dx_docs/" + ctx.label.name)

def _real_rust_docs_aspect_impl(target, ctx):
    shard = _rust_docs_shard(target, ctx)
    if not shard:
        return []
    return [OutputGroupInfo(
        dx_results = depset([shard.shard]),
        dx_docs_shards = depset([shard.shard]),
        rustdoc_json = depset(shard.outputs["rustdoc_json"].to_list()),
        srcs_manifest = depset(shard.outputs["srcs_manifest"].to_list()),
    )]

real_rust_docs_aspect = aspect(
    implementation = _real_rust_docs_aspect_impl,
    doc = "Puts the Rust docs shard of a target in dx_results and dx_docs_shards.",
    attrs = {
        "_extract": attr.label(
            cfg = "exec",
            default = Label("//tools/docs/rust/extract:rust_extract"),
            executable = True,
        ),
        "_process_wrapper": attr.label(
            default = Label("@rules_rust//util/process_wrapper"),
            cfg = "exec",
            executable = True,
        ),
        "_error_format": attr.label(default = Label("@rules_rust//rust/settings:error_format")),
    },
    fragments = ["cpp"],
    requires = [rust_doc_inputs_aspect],
    toolchains = [
        config_common.toolchain_type(_RUST_DOCS_TOOLCHAIN, mandatory = False),
        config_common.toolchain_type(_CC_DOCS_TOOLCHAIN, mandatory = False),
    ],
)
