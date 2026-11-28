"""Protocol Buffers real quality aspects (buf)."""

load("@rules_proto//proto:defs.bzl", "ProtoInfo")
load("//core/quality:family_hooks.bzl", "upstream_files")
load("//core/quality:real_core.bzl", "real_config", "real_context", "real_make_aspect", "real_make_probe_aspect", "real_resolve_list")

visibility(["//core/...", "//tools/..."])

TOOLS = {
    "buf": {
        "attr": attr.label(default = "//config:tool_buf", allow_single_file = True, cfg = "exec"),
        "kind": "file",
        "resources": "light",
        "shard": "protobuf",
    },
}

_BUF_FORMAT_ATTRS = {
    "_dx": attr.label(default = "@dx_prebuilt//dx:buf_diff", executable = True, cfg = "exec", doc = "The dx whose buf_diff subcommand buf format runs as diff."),
}

SHARD_SPECS = {
    "real_protobuf_format": {
        "capability": "format",
        "doc": "Additive Protocol Buffers format family aspect (buf format).",
        "extra_attrs": _BUF_FORMAT_ATTRS,
        "shard": "protobuf",
    },
    "real_protobuf_typecheck": {
        "capability": "typecheck",
        "doc": "Additive Protocol Buffers typecheck family aspect (buf build).",
        "shard": "protobuf",
    },
    "real_protobuf_lint": {
        "capability": "lint",
        "doc": "Additive Protocol Buffers lint family aspect (buf lint).",
        "shard": "protobuf",
    },
}

def _buf_format_context(ctx, capability):
    """Returns the inputs and options that put `dx buf_diff` on buf format's PATH as diff."""
    if capability != "format":
        return {}
    return {
        "inputs": [ctx.executable._dx],
        "options": {"path_tools": [("diff", ctx.executable._dx, ["buf_diff"])]},
    }

def _exec_line(f):
    return f.path + "\t" + f.path

def _buf_hook(target, ctx, _resolved, capability, _what, _shared):
    if ProtoInfo not in target:
        return real_context(**_buf_format_context(ctx, capability))
    info = target[ProtoInfo]
    return real_context(resolve_lists = [real_resolve_list(info.transitive_sources, map_each = _exec_line, strip_roots = info.transitive_proto_path)], **_buf_format_context(ctx, capability))

def _proto_sources(target, ctx):
    if ProtoInfo not in target:
        return None
    return upstream_files(getattr(ctx.rule.files, "srcs", None), "proto")

HOOK_ORDER = ["buf"]

HOOKS = {"buf": _buf_hook}

RECOGNIZERS = [_proto_sources]

_CFG = real_config(
    tools = TOOLS,
    shard_specs = SHARD_SPECS,
    hook_order = HOOK_ORDER,
    context_hooks = HOOKS,
    source_recognizers = RECOGNIZERS,
)

real_protobuf_format_aspect = real_make_aspect(_CFG, "real_protobuf_format")

real_protobuf_lint_aspect = real_make_aspect(_CFG, "real_protobuf_lint")

real_protobuf_typecheck_aspect = real_make_aspect(_CFG, "real_protobuf_typecheck")

ProtobufShardProbeInfo = provider(
    doc = "Shard names the Protocol Buffers shard probe found for one target and capability.",
    fields = {
        "capability": "lint, format or typecheck",
        "shards": "sorted shard names whose tool filter keeps a resolved stage",
    },
)

QUALITY_CFG = _CFG

real_protobuf_format_probe_aspect = real_make_probe_aspect(_CFG, "format", "protobuf", ProtobufShardProbeInfo)

real_protobuf_lint_probe_aspect = real_make_probe_aspect(_CFG, "lint", "protobuf", ProtobufShardProbeInfo)

real_protobuf_typecheck_probe_aspect = real_make_probe_aspect(_CFG, "typecheck", "protobuf", ProtobufShardProbeInfo)
