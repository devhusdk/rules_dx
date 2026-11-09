"""Schema and code generation real quality aspects (Protocol Buffers, Cap'n Proto, FlatBuffers, Avro, Thrift)."""

load("@rules_proto//proto:defs.bzl", "ProtoInfo")
load("//avro/rules:providers.bzl", "AvroInfo")
load("//capnproto/rules:providers.bzl", "CapnpInfo")
load("//flatbuffers/rules:defs.bzl", "FlatBuffersInfo")
load("//quality:family_hooks.bzl", "upstream_files")
load("//quality:real_core.bzl", "real_config", "real_context", "real_make_aspect", "real_make_probe_aspect")
load("//thrift/rules:defs.bzl", "ThriftInfo")

TOOLS = {
    "buf": {
        "attr": attr.label(default = "@dx_tools//:buf", allow_single_file = True, cfg = "exec"),
        "env": [["PATH", "/usr/bin:/bin"]],
        "kind": "file",
        "resources": "light",
        "shard": "protobuf",
    },
    "capnp": {
        "attr": attr.label(default = "//config:tool_capnp", allow_files = True, cfg = "exec", executable = True),
        "fallback": " to a capnp binary",
        "kind": "setting",
        "resources": "light",
        "shard": "capnp",
    },
    "flatc": {
        "attr": attr.label(default = "//config:tool_flatc", allow_files = True, cfg = "exec", executable = True),
        "fallback": " to a flatc binary",
        "kind": "setting",
        "resources": "light",
        "shard": "flatbuffers",
    },
    "avro_tools": {
        "attr": attr.label(default = "//config:tool_avro", allow_files = True, cfg = "exec", executable = True),
        "fallback": " to an avro-tools binary",
        "kind": "setting",
        "resources": "light",
        "shard": "avro",
    },
    "thrift": {
        "attr": attr.label(default = "//config:tool_thrift", allow_files = True, cfg = "exec", executable = True),
        "fallback": " to a thrift binary",
        "kind": "setting",
        "resources": "light",
        "shard": "thrift",
    },
}

SHARD_SPECS = {
    "real_capnp_lint": {
        "capability": "lint",
        "doc": "Additive Cap'n Proto lint family aspect (capnp).",
        "shard": "capnp",
        "suffix": "-capnp",
    },
    "real_capnp_typecheck": {
        "capability": "typecheck",
        "doc": "Additive Cap'n Proto typecheck family aspect (capnp errors only).",
        "shard": "capnp",
        "suffix": "-capnp",
    },
    "real_flatbuffers_lint": {
        "capability": "lint",
        "doc": "Additive FlatBuffers lint family aspect (flatc).",
        "shard": "flatbuffers",
        "suffix": "-flatbuffers",
    },
    "real_flatbuffers_typecheck": {
        "capability": "typecheck",
        "doc": "Additive FlatBuffers typecheck family aspect (flatc errors only).",
        "shard": "flatbuffers",
        "suffix": "-flatbuffers",
    },
    "real_avro_lint": {
        "capability": "lint",
        "doc": "Additive Avro lint family aspect (avro-tools).",
        "shard": "avro",
        "suffix": "-avro",
    },
    "real_avro_typecheck": {
        "capability": "typecheck",
        "doc": "Additive Avro typecheck family aspect (avro-tools errors only).",
        "shard": "avro",
        "suffix": "-avro",
    },
    "real_thrift_lint": {
        "capability": "lint",
        "doc": "Additive Thrift lint family aspect (thrift).",
        "shard": "thrift",
        "suffix": "-thrift",
    },
    "real_thrift_typecheck": {
        "capability": "typecheck",
        "doc": "Additive Thrift typecheck family aspect (thrift errors only).",
        "shard": "thrift",
        "suffix": "-thrift",
    },
    "real_protobuf_format": {
        "capability": "format",
        "doc": "Additive Protocol Buffers format family aspect (buf format).",
        "shard": "protobuf",
        "suffix": "-protobuf",
    },
    "real_protobuf_typecheck": {
        "capability": "typecheck",
        "doc": "Additive Protocol Buffers typecheck family aspect (buf build).",
        "shard": "protobuf",
        "suffix": "-protobuf",
    },
    "real_protobuf_lint": {
        "capability": "lint",
        "doc": "Additive Protocol Buffers lint family aspect (buf lint).",
        "shard": "protobuf",
        "suffix": "-protobuf",
    },
}

def _capnp_closure(target):
    return [(i.path, i.file) for i in target[CapnpInfo].transitive_imports.to_list()]

def _flatc_closure(target):
    staged = []
    for f in target[FlatBuffersInfo].transitive_srcs.to_list():
        ws_path = f.short_path
        if ws_path.startswith("../"):
            ws_path = "_external/" + ws_path[3:]
        if ".." in ws_path.split("/"):
            continue
        staged.append((ws_path, f))
    return staged

def _avro_closure(target):
    return [(f.short_path, f) for f in target[AvroInfo].transitive_srcs.to_list() if ".." not in f.short_path.split("/")]

def _thrift_closure(target):
    return [(i.path, i.file) for i in target[ThriftInfo].transitive_includes.to_list()]

# Tools whose declared import closure is staged next to the checked files:
# tool -> (provider, closure(target) -> [(staged path, File)], strict). A strict
# tool fails when two different Files claim one staged path, because the
# compiler would see a different file than the one declared.
_CLOSURE_STAGING = {
    "capnp": (CapnpInfo, _capnp_closure, True),
    "flatc": (FlatBuffersInfo, _flatc_closure, False),
    "thrift": (ThriftInfo, _thrift_closure, True),
    "avro_tools": (AvroInfo, _avro_closure, True),
}

def _closure_hook(tool):
    (provider, closure_files, strict) = _CLOSURE_STAGING[tool]

    def _hook(target, _ctx, _resolved, _capability, _what, _shared):
        if provider not in target:
            return real_context()
        return real_context(resolve_pairs = [(ws_path, f, strict) for (ws_path, f) in closure_files(target)])

    return _hook

def _buf_hook(target, _ctx, _resolved, _capability, _what, _shared):
    if ProtoInfo not in target:
        return real_context()
    pairs = []
    proto_roots = sorted([r for r in target[ProtoInfo].transitive_proto_path.to_list() if r != "."], key = lambda r: -len(r))
    for f in target[ProtoInfo].transitive_sources.to_list():
        ws_path = f.path
        for root in proto_roots:
            if ws_path.startswith(root + "/"):
                ws_path = ws_path[len(root) + 1:]
                break
        if ".." in ws_path.split("/"):
            continue
        pairs.append((ws_path, f, False))
    return real_context(resolve_pairs = pairs)

def _proto_sources(target, ctx):
    if ProtoInfo not in target:
        return None
    return upstream_files(getattr(ctx.rule.files, "srcs", None), "proto")

HOOK_ORDER = ["capnp", "flatc", "thrift", "avro_tools", "buf"]

HOOKS = {"buf": _buf_hook} | {tool: _closure_hook(tool) for tool in _CLOSURE_STAGING.keys()}

RECOGNIZERS = [_proto_sources]

_CFG = real_config(
    tools = TOOLS,
    shard_specs = SHARD_SPECS,
    hook_order = HOOK_ORDER,
    context_hooks = HOOKS,
    source_recognizers = RECOGNIZERS,
    harness_sources = lambda _target, _capability, direct_sources: direct_sources,
    toolchain = None,
    check_policy = False,
)

real_avro_lint_aspect = real_make_aspect(_CFG, "real_avro_lint")

real_avro_typecheck_aspect = real_make_aspect(_CFG, "real_avro_typecheck")

real_capnp_lint_aspect = real_make_aspect(_CFG, "real_capnp_lint")

real_capnp_typecheck_aspect = real_make_aspect(_CFG, "real_capnp_typecheck")

real_flatbuffers_lint_aspect = real_make_aspect(_CFG, "real_flatbuffers_lint")

real_flatbuffers_typecheck_aspect = real_make_aspect(_CFG, "real_flatbuffers_typecheck")

real_protobuf_format_aspect = real_make_aspect(_CFG, "real_protobuf_format")

real_protobuf_lint_aspect = real_make_aspect(_CFG, "real_protobuf_lint")

real_protobuf_typecheck_aspect = real_make_aspect(_CFG, "real_protobuf_typecheck")

real_thrift_lint_aspect = real_make_aspect(_CFG, "real_thrift_lint")

real_thrift_typecheck_aspect = real_make_aspect(_CFG, "real_thrift_typecheck")

GenerationShardProbeInfo = provider(
    doc = "Shard names the generation shard probe found for one target and capability.",
    fields = {
        "capability": "lint, format or typecheck",
        "shards": "sorted shard names whose tool filter keeps a resolved stage",
    },
)

real_generation_format_probe_aspect = real_make_probe_aspect(_CFG, "format", "generation", GenerationShardProbeInfo)

real_generation_lint_probe_aspect = real_make_probe_aspect(_CFG, "lint", "generation", GenerationShardProbeInfo)

real_generation_typecheck_probe_aspect = real_make_probe_aspect(_CFG, "typecheck", "generation", GenerationShardProbeInfo)
