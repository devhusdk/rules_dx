"""Python real quality aspects."""

load("@rules_py//py:defs.bzl", _PyInfo = "PyInfo")
load("//quality:real_core.bzl", "real_config", "real_context", "real_make_aspect", "real_make_probe_aspect")
load("//quality:unused_roots.bzl", "DxUnusedRootsInfo", "unused_roots_aspect")

TOOLS = {
    "flake8": {
        "attr": attr.label(default = "//quality/tools/python:flake8", cfg = "exec", executable = True),
        "kind": "executable",
        "resources": "medium",
        "runfiles_env": True,
        "shard": "py",
    },
    "pydoclint": {
        "attr": attr.label(default = "//quality/tools/python:pydoclint", cfg = "exec", executable = True),
        "kind": "executable",
        "resources": "medium",
        "runfiles_env": True,
        "shard": "py",
    },
    "pylint": {
        "attr": attr.label(default = "//quality/tools/python:pylint", cfg = "exec", executable = True),
        "kind": "executable",
        "resources": "medium",
        "runfiles_env": True,
        "shard": "py",
    },
    "vulture": {
        "attr": attr.label(default = "//quality/tools/python:vulture", cfg = "exec", executable = True),
        "capabilities": ["unused"],
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
        "doc": "Additive Python lint family aspect (flake8/pylint/pydoclint).",
        "shard": "py",
        "suffix": "-py",
    },
    "real_python_unused": {
        "capability": "unused",
        "doc": "Additive Python unused-code family aspect (vulture over the target's whole closure).",
        "requires": [unused_roots_aspect],
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

def _python_files(files):
    return [f for f in files if f.extension == "py"]

def _roots_paths(files):
    return sorted({f.short_path: True for f in _python_files(files.to_list())}.keys())

def _vulture_hook(target, ctx, _resolved, _capability, _what, _shared):
    if DxUnusedRootsInfo not in target:
        return real_context()
    roots = target[DxUnusedRootsInfo]
    out = ctx.actions.declare_file(target.label.name + "-dx-unused-roots.json")
    ctx.actions.write(out, json.encode({
        "keep": _roots_paths(roots.keep),
        "mains": _roots_paths(roots.mains),
        "public": _roots_paths(roots.public),
    }))
    return real_context(tool_files = [("dx-unused/roots.json", out)], inputs = [out])

def _harness_sources(target, capability, direct_sources):
    if capability != "unused" or DxUnusedRootsInfo not in target:
        return direct_sources
    closure = _python_files(target[DxUnusedRootsInfo].sources.to_list())
    return {"python": depset(closure)} if closure else direct_sources

HOOK_ORDER = ["ty", "vulture"]

HOOKS = {"ty": _ty_hook, "vulture": _vulture_hook}

_CFG = real_config(
    tools = TOOLS,
    shard_specs = SHARD_SPECS,
    hook_order = HOOK_ORDER,
    context_hooks = HOOKS,
    source_recognizers = [],
    harness_sources = _harness_sources,
    toolchain = None,
    check_policy = False,
)

real_python_format_aspect = real_make_aspect(_CFG, "real_python_format")

real_python_lint_aspect = real_make_aspect(_CFG, "real_python_lint")

real_python_unused_aspect = real_make_aspect(_CFG, "real_python_unused")

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
