"""Core real quality aspects."""

load("//quality:real_core.bzl", "real_config", "real_context", "real_make_aspect")
load("//quality:sources.bzl", "QualityToolsInfo")

TOOLS = {
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
    "markdown_check": {
        "attr": attr.label(default = "//quality/markdown:quality_markdown", allow_single_file = True, cfg = "exec"),
        "kind": "file",
        "resources": "light",
        "shard": "core",
    },
    "notebook_outputs": {
        "attr": attr.label(default = "//quality/notebook:quality_notebook", allow_single_file = True, cfg = "exec"),
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
    "asciidoc": ("asciidoctor", "asciidoc_library", "@rules_dx//asciidoc/rules:defs.bzl"),
    "rst": ("rst2html", "rst_library", "@rules_dx//rst/rules:defs.bzl"),
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

HOOK_ORDER = ["vale"]

HOOKS = {
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
