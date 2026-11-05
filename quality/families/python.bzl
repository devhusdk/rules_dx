"""Python real quality aspects."""

load("@rules_py//py:defs.bzl", _PyInfo = "PyInfo")
load("//quality:real_core.bzl", "real_config", "real_context", "real_make_aspect")

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
        "doc": "Additive Python lint family aspect (flake8/pylint/pydoclint/vulture).",
        "shard": "py",
        "suffix": "-py",
    },
    "real_python_unused": {
        "capability": "unused",
        "doc": "Additive Python unused-code family aspect (vulture).",
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

real_python_unused_aspect = real_make_aspect(_CFG, "real_python_unused")

real_python_typecheck_aspect = real_make_aspect(_CFG, "real_python_typecheck")
