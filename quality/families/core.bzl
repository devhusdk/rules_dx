"""Core real quality aspects."""

load("//quality:family_hooks.bzl", "default_files_hook")
load("//quality:real_core.bzl", "real_config", "real_context", "real_make_aspect")
load("//quality:sources.bzl", "QualityToolsInfo")

TOOLS = {
    "actionlint": {
        "attr": attr.label(default = "//config:tool_actionlint", allow_single_file = True, cfg = "exec"),
        "kind": "file",
        "resources": "light",
        "shard": "core",
    },
    "biome": {
        "attr": attr.label(default = "@dx_tools//:biome", allow_single_file = True, cfg = "exec"),
        "kind": "file",
        "resources": "light",
        "shard": "core",
    },
    "buildifier": {
        "attr": attr.label(default = "@dx_tools//:buildifier", allow_single_file = True, cfg = "exec"),
        "kind": "file",
        "resources": "light",
        "shard": "core",
    },
    "docstrfmt": {
        "attr": attr.label(default = "//quality/tools/python:docstrfmt", cfg = "exec", executable = True),
        "capabilities": ["format"],
        "kind": "executable",
        "resources": "light",
        "runfiles_env": True,
        "shard": "core",
    },
    "lychee": {
        "attr": attr.label(default = "@dx_tools//:lychee", allow_single_file = True, cfg = "exec"),
        "capabilities": ["lint"],
        "kind": "file",
        "resources": "light",
        "shard": "core",
    },
    "markdown_check": {
        "attr": attr.label(default = "@dx_prebuilt//:quality_markdown", allow_single_file = True, cfg = "exec"),
        "kind": "file",
        "resources": "light",
        "shard": "core",
    },
    "notebook_outputs": {
        "attr": attr.label(default = "@dx_prebuilt//:quality_notebook", allow_single_file = True, cfg = "exec"),
        "kind": "file",
        "resources": "light",
        "shard": "core",
    },
    "taplo": {
        "attr": attr.label(default = "@dx_tools//:taplo", allow_single_file = True, cfg = "exec"),
        "kind": "file",
        "resources": "light",
        "shard": "core",
    },
    "vale": {
        "attr": attr.label(default = "@dx_tools//:vale", allow_single_file = True, cfg = "exec"),
        "kind": "file",
        "resources": "light",
        "shard": "core",
    },
    "zizmor": {
        "attr": attr.label(default = "@dx_tools//:zizmor", allow_single_file = True, cfg = "exec"),
        "kind": "file",
        "resources": "light",
        "shard": "core",
    },
}

_ACTIONLINT_ATTRS = {
    "_actionlint_shellcheck": attr.label(default = "@dx_tools//:shellcheck", allow_single_file = True, cfg = "exec", doc = "The pinned shellcheck actionlint runs on run: blocks."),
}

SHARD_SPECS = {
    "real_format": {
        "capability": "format",
        "doc": "Registers the exact-input real format pipeline action in dx_results.",
        "shard": "core",
        "shared_attrs": True,
        "suffix": "",
    },
    "real_lint": {
        "capability": "lint",
        "doc": "Registers the exact-input real lint pipeline action in dx_results.",
        "extra_attrs": _ACTIONLINT_ATTRS,
        "shard": "core",
        "shared_attrs": True,
        "suffix": "",
    },
    "real_typecheck": {
        "capability": "typecheck",
        "doc": "Registers the exact-input real typecheck pipeline action in dx_results.",
        "shard": "core",
        "shared_attrs": True,
        "suffix": "",
    },
}

_VALE_CONVERTERS = {
    "asciidoc": ("asciidoctor", "asciidoc_library", "@rules_dx//asciidoc:defs.bzl"),
    "rst": ("rst2html", "rst_library", "@rules_dx//rst:defs.bzl"),
}

def _vale_converters(target, stage_classes, what):
    """Returns (name, FilesToRunProvider) for each converter the stage's classes need."""
    found = []
    for cls in sorted(_VALE_CONVERTERS.keys()):
        if cls not in stage_classes:
            continue
        name, owner, defs = _VALE_CONVERTERS[cls]
        if QualityToolsInfo not in target or name not in target[QualityToolsInfo].tools:
            fail(what + ": vale: " + cls + " files need " + name + "; own them with " + owner + " from " + defs)
        found.append((name, target[QualityToolsInfo].tools[name]))
    return found

def _vale_hook(target, _ctx, resolved, _capability, what, _shared):
    converters = []
    for stage in resolved:
        if stage["tool"] == "vale":
            for converter in _vale_converters(target, stage["classes"], what):
                if converter[0] not in [c[0] for c in converters]:
                    converters.append(converter)
    return real_context(
        inputs = [converter.executable for (_, converter) in converters],
        run_tools = [converter for (_, converter) in converters],
        options = {"path_tools": [(name, converter.executable) for (name, converter) in converters]},
    )

def _actionlint_hook(_target, ctx, _resolved, _capability, _what, _shared):
    return real_context(
        inputs = [ctx.file._actionlint_shellcheck],
        options = {"path_tools": [("shellcheck", ctx.file._actionlint_shellcheck)]},
    )

HOOK_ORDER = ["actionlint", "taplo", "vale"]

HOOKS = {
    "actionlint": _actionlint_hook,
    "taplo": default_files_hook(["json"], only_capability = "typecheck"),
    "vale": _vale_hook,
}

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

real_lint_aspect = real_make_aspect(_CFG, "real_lint")

real_format_aspect = real_make_aspect(_CFG, "real_format")

real_typecheck_aspect = real_make_aspect(_CFG, "real_typecheck")
