"""OCaml real quality aspects."""

load("//ocaml/private:providers.bzl", "OCamlCmtInfo")
load("//quality:compile_check.bzl", "compile_check_aspect", "compile_check_hook")
load("//quality:family_hooks.bzl", "stage")
load("//quality:pipeline.bzl", "real_request_tool_env")
load("//quality:real_core.bzl", "real_config", "real_context", "real_make_aspect", "real_make_probe_aspect")

TOOLS = {
    "ocaml": {
        "kind": "upstream",
        "resources": "heavy",
        "shard": "ocaml",
    },
    "zanuda": {
        "attr": attr.label(default = "//ocaml/private/quality:zanuda_tool", cfg = "exec", executable = True),
        "kind": "executable",
        "resources": "light",
        "shard": "ocaml",
    },
    "topiary": {
        "attr": attr.label(default = "//rust/private/quality/tools:topiary_tool", cfg = "exec", executable = True),
        "kind": "executable",
        "resources": "light",
        "shard": "ocaml",
    },
}

SHARD_SPECS = {
    "real_ocaml_lint": {
        "capability": "lint",
        "doc": "Additive OCaml lint family aspect (zanuda; target-coupled via OCamlCmtInfo).",
        "extra_attrs": {
            "_ocaml_stdlib": attr.label(
                default = "@dx_ocaml_toolchains//:stdlib_cmis",
                cfg = "exec",
                doc = "The compiler's standard library interfaces, which zanuda type checks against.",
            ),
        },
        "shard": "ocaml",
        "suffix": "-ocaml",
    },
    "real_ocaml_format": {
        "capability": "format",
        "doc": "Additive OCaml format family aspect (topiary).",
        "shard": "ocaml",
        "suffix": "-ocaml",
    },
    "real_ocaml_typecheck": {
        "capability": "typecheck",
        "doc": "Additive OCaml typecheck family aspect (ocamlc -stop-after typing replay of the compile action).",
        "requires": [compile_check_aspect],
        "shard": "ocaml",
        "suffix": "-ocaml",
    },
}

def _zanuda_hook(target, ctx, resolved, _capability, what, _shared):
    # zanuda reads the typed trees the compile wrote. A target that owns OCaml sources but
    # carries no OCamlCmtInfo, or one that owns a source with no tree, would otherwise lint
    # nothing and look clean, so both fail the analysis.
    if OCamlCmtInfo not in target:
        fail(what + ": zanuda needs OCamlCmtInfo, which only the dx ocaml_module and ocaml_signature rules provide")
    listed = {}
    for entry in target[OCamlCmtInfo].owned:
        if entry.cmt and entry.source:
            listed[entry.source.short_path] = ("impl", entry.cmt)
        if entry.cmti and entry.signature:
            listed[entry.signature.short_path] = ("intf", entry.cmti)
    tool_files = []
    inputs = []
    for step in resolved:
        if step["tool"] != "zanuda":
            continue
        for ws_path in step["sources"]:
            if ws_path not in listed:
                fail(what + ": zanuda has no .cmt or .cmti for " + ws_path + "; was the target compiled with -no-bin-annot?")
            (kind, tree) = listed[ws_path]
            stage(tool_files, inputs, "dx-zanuda/%s/%s.%s" % (kind, ws_path, tree.extension), tree)
    stdlib = ctx.files._ocaml_stdlib
    inputs.extend(stdlib)

    # Relative to the execution root, like the binary; the runner makes it absolute.
    env = [real_request_tool_env("OCAMLLIB", stdlib[0].dirname)] if len(stdlib) > 0 else []
    return real_context(tool_files = tool_files, inputs = inputs, env = env)

HOOK_ORDER = ["ocaml", "zanuda"]

HOOKS = {"ocaml": compile_check_hook("ocaml"), "zanuda": _zanuda_hook}

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

real_ocaml_format_aspect = real_make_aspect(_CFG, "real_ocaml_format")

real_ocaml_lint_aspect = real_make_aspect(_CFG, "real_ocaml_lint")

real_ocaml_typecheck_aspect = real_make_aspect(_CFG, "real_ocaml_typecheck")

OcamlShardProbeInfo = provider(
    doc = "Shard names the ocaml shard probe found for one target and capability.",
    fields = {
        "capability": "lint, format or typecheck",
        "shards": "sorted shard names whose tool filter keeps a resolved stage",
    },
)

real_ocaml_format_probe_aspect = real_make_probe_aspect(_CFG, "format", "ocaml", OcamlShardProbeInfo)

real_ocaml_lint_probe_aspect = real_make_probe_aspect(_CFG, "lint", "ocaml", OcamlShardProbeInfo)

real_ocaml_typecheck_probe_aspect = real_make_probe_aspect(_CFG, "typecheck", "ocaml", OcamlShardProbeInfo)
