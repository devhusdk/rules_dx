"""Python real quality aspects."""

load("@rules_py//py:defs.bzl", _PyInfo = "PyInfo")
load("//core/quality:real_core.bzl", "real_config", "real_context", "real_make_aspect", "real_make_probe_aspect")

TOOLS = {
    "pydoclint": {
        "attr": attr.label(default = "//core/quality/tools/python:pydoclint", cfg = "exec", executable = True),
        "kind": "executable",
        "resources": "medium",
        "runfiles_env": True,
        "shard": "py",
    },
    "ruff": {
        "attr": attr.label(default = "//config:tool_ruff", allow_single_file = True, cfg = "exec"),
        "capabilities": ["format", "lint"],
        "kind": "file",
        "resources": "light",
        "shard": "py",
    },
    "ty": {
        "attr": attr.label(default = "@dx_tools//:ty", allow_single_file = True, cfg = "exec"),
        "kind": "file",
        "resources": "medium",
        "shard": "py",
    },
}

SHARD_SPECS = {
    "real_python_format": {
        "capability": "format",
        "doc": "Additive Python format family aspect (ruff format).",
        "shard": "py",
        "suffix": "-py",
    },
    "real_python_typecheck": {
        "capability": "typecheck",
        "doc": "Additive Python typecheck family aspect (ty).",
        "shard": "py",
        "suffix": "-py",
    },
    "real_python_lint": {
        "capability": "lint",
        "doc": "Additive Python lint family aspect (pydoclint).",
        "shard": "py",
        "suffix": "-py",
    },
}

def _ty_hook(target, _ctx, _resolved, _capability, _what, _shared):
    if _PyInfo not in target:
        return real_context()
    transitive = getattr(target[_PyInfo], "transitive_sources", None)
    if transitive == None:
        return real_context()
    pairs = []
    search_dirs = []
    for f in transitive.to_list():
        ws_path = f.short_path
        if ".." not in ws_path.split("/") and (ws_path.endswith(".py") or ws_path.endswith(".pyi")):
            pairs.append((ws_path, f, False))
    for f in transitive.to_list():
        if f.is_directory and f.short_path.endswith(".install"):
            search_dirs.append(f)
    return real_context(early_inputs = search_dirs, resolve_pairs = pairs, options = {"search_paths": search_dirs})

HOOK_ORDER = ["ty"]

HOOKS = {"ty": _ty_hook}

_CFG = real_config(
    tools = TOOLS,
    shard_specs = SHARD_SPECS,
    hook_order = HOOK_ORDER,
    context_hooks = HOOKS,
    source_recognizers = [],
    harness_sources = lambda _target, _capability, direct_sources: direct_sources,
    toolchain = None,
    check_policy = False,
)

real_python_format_aspect = real_make_aspect(_CFG, "real_python_format")

real_python_lint_aspect = real_make_aspect(_CFG, "real_python_lint")

real_python_typecheck_aspect = real_make_aspect(_CFG, "real_python_typecheck")

PythonShardProbeInfo = provider(
    doc = "Shard names the python shard probe found for one target and capability.",
    fields = {
        "capability": "lint, format or typecheck",
        "shards": "sorted shard names whose tool filter keeps a resolved stage",
    },
)

real_python_format_probe_aspect = real_make_probe_aspect(_CFG, "format", "python", PythonShardProbeInfo)

real_python_lint_probe_aspect = real_make_probe_aspect(_CFG, "lint", "python", PythonShardProbeInfo)

real_python_typecheck_probe_aspect = real_make_probe_aspect(_CFG, "typecheck", "python", PythonShardProbeInfo)
