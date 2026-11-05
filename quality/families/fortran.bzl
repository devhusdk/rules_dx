"""Fortran real quality aspects."""

load("//quality:compile_check.bzl", "compile_check_aspect", "compile_check_hook")
load("//quality:real_core.bzl", "real_config", "real_make_aspect", "real_make_probe_aspect")

TOOLS = {
    "ffmt": {
        "attr": attr.label(default = "@dx_tools//:ffmt", allow_single_file = True, cfg = "exec"),
        "kind": "file",
        "resources": "light",
        "shard": "fortran",
    },
    "flang": {
        "kind": "upstream",
        "resources": "heavy",
        "shard": "fortran",
    },
    "fortitude": {
        "attr": attr.label(default = "@dx_tools//:fortitude", allow_single_file = True, cfg = "exec"),
        "kind": "file",
        "resources": "light",
        "shard": "fortran",
    },
}

SHARD_SPECS = {
    "real_fortran_format": {
        "capability": "format",
        "doc": "Additive Fortran format family aspect (ffmt).",
        "shard": "fortran",
        "suffix": "-fortran",
    },
    "real_fortran_lint": {
        "capability": "lint",
        "doc": "Additive Fortran lint family aspect (fortitude).",
        "shard": "fortran",
        "suffix": "-fortran",
    },
    "real_fortran_typecheck": {
        "capability": "typecheck",
        "doc": "Additive Fortran typecheck family aspect (flang replay of the compile action).",
        "requires": [compile_check_aspect],
        "shard": "fortran",
        "suffix": "-fortran",
    },
}

_CFG = real_config(
    tools = TOOLS,
    shard_specs = SHARD_SPECS,
    hook_order = ["flang"],
    context_hooks = {"flang": compile_check_hook("flang")},
    source_recognizers = [],
    harness_sources = lambda _target, _capability, direct_sources: direct_sources,
    toolchain = None,
    check_policy = False,
)

real_fortran_format_aspect = real_make_aspect(_CFG, "real_fortran_format")

real_fortran_lint_aspect = real_make_aspect(_CFG, "real_fortran_lint")

real_fortran_typecheck_aspect = real_make_aspect(_CFG, "real_fortran_typecheck")

FortranShardProbeInfo = provider(
    doc = "Shard names the fortran shard probe found for one target and capability.",
    fields = {
        "capability": "lint, format or typecheck",
        "shards": "sorted shard names whose tool filter keeps a resolved stage",
    },
)

real_fortran_format_probe_aspect = real_make_probe_aspect(_CFG, "format", "fortran", FortranShardProbeInfo)

real_fortran_lint_probe_aspect = real_make_probe_aspect(_CFG, "lint", "fortran", FortranShardProbeInfo)

real_fortran_typecheck_probe_aspect = real_make_probe_aspect(_CFG, "typecheck", "fortran", FortranShardProbeInfo)
