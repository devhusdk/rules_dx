"""Schema and code generation real quality aspects (Cap'n Proto, FlatBuffers, Avro, Thrift)."""

load("//avro/private:providers.bzl", "AvroInfo")
load("//capnproto/private:providers.bzl", "CapnprotoInfo")
load("//core/quality:real_core.bzl", "real_config", "real_context", "real_make_aspect", "real_make_probe_aspect", "real_resolve_list")
load("//flatbuffers:defs.bzl", "FlatBuffersInfo")
load("//thrift:defs.bzl", "ThriftInfo")

visibility(["//core/...", "//tools/..."])

TOOLS = {
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
    },
    "real_capnp_typecheck": {
        "capability": "typecheck",
        "doc": "Additive Cap'n Proto typecheck family aspect (capnp errors only).",
        "shard": "capnp",
    },
    "real_flatbuffers_lint": {
        "capability": "lint",
        "doc": "Additive FlatBuffers lint family aspect (flatc).",
        "shard": "flatbuffers",
    },
    "real_flatbuffers_typecheck": {
        "capability": "typecheck",
        "doc": "Additive FlatBuffers typecheck family aspect (flatc errors only).",
        "shard": "flatbuffers",
    },
    "real_avro_lint": {
        "capability": "lint",
        "doc": "Additive Avro lint family aspect (avro-tools).",
        "shard": "avro",
    },
    "real_avro_typecheck": {
        "capability": "typecheck",
        "doc": "Additive Avro typecheck family aspect (avro-tools errors only).",
        "shard": "avro",
    },
    "real_thrift_lint": {
        "capability": "lint",
        "doc": "Additive Thrift lint family aspect (thrift).",
        "shard": "thrift",
    },
    "real_thrift_typecheck": {
        "capability": "typecheck",
        "doc": "Additive Thrift typecheck family aspect (thrift errors only).",
        "shard": "thrift",
    },
}

def _capnp_closure(target):
    return [(i.path, i.file) for i in target[CapnprotoInfo].transitive_imports.to_list()]

def _flatc_line(f):
    ws_path = f.short_path
    if ws_path.startswith("../"):
        ws_path = "_external/" + ws_path[3:]
    if ".." in ws_path.split("/"):
        return None
    return ws_path + "\t" + f.path

def _flatc_hook(target, _ctx, _resolved, _capability, _what, _shared):
    if FlatBuffersInfo not in target:
        return real_context()
    return real_context(resolve_lists = [real_resolve_list(target[FlatBuffersInfo].transitive_srcs, map_each = _flatc_line)])

def _avro_closure(target):
    return [(f.short_path, f) for f in target[AvroInfo].transitive_srcs.to_list() if ".." not in f.short_path.split("/")]

def _thrift_closure(target):
    return [(i.path, i.file) for i in target[ThriftInfo].transitive_includes.to_list()]

_CLOSURE_STAGING = {
    "capnp": (CapnprotoInfo, _capnp_closure, True),
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

HOOK_ORDER = ["capnp", "flatc", "thrift", "avro_tools"]

HOOKS = {"flatc": _flatc_hook} | {tool: _closure_hook(tool) for tool in _CLOSURE_STAGING.keys()}

_CFG = real_config(
    tools = TOOLS,
    shard_specs = SHARD_SPECS,
    hook_order = HOOK_ORDER,
    context_hooks = HOOKS,
)

real_avro_lint_aspect = real_make_aspect(_CFG, "real_avro_lint")

real_avro_typecheck_aspect = real_make_aspect(_CFG, "real_avro_typecheck")

real_capnp_lint_aspect = real_make_aspect(_CFG, "real_capnp_lint")

real_capnp_typecheck_aspect = real_make_aspect(_CFG, "real_capnp_typecheck")

real_flatbuffers_lint_aspect = real_make_aspect(_CFG, "real_flatbuffers_lint")

real_flatbuffers_typecheck_aspect = real_make_aspect(_CFG, "real_flatbuffers_typecheck")

real_thrift_lint_aspect = real_make_aspect(_CFG, "real_thrift_lint")

real_thrift_typecheck_aspect = real_make_aspect(_CFG, "real_thrift_typecheck")

GenerationShardProbeInfo = provider(
    doc = "Shard names the generation shard probe found for one target and capability.",
    fields = {
        "capability": "lint or typecheck",
        "shards": "sorted shard names whose tool filter keeps a resolved stage",
    },
)

QUALITY_CFG = _CFG

real_generation_lint_probe_aspect = real_make_probe_aspect(_CFG, "lint", "generation", GenerationShardProbeInfo)

real_generation_typecheck_probe_aspect = real_make_probe_aspect(_CFG, "typecheck", "generation", GenerationShardProbeInfo)
