"""Perl real quality aspects."""

load("//core/quality:real_core.bzl", "real_config", "real_make_aspect", "real_make_probe_aspect")

TOOLS = {
    "perlcritic": {
        "attr": attr.label(default = "//config:tool_perlcritic", allow_files = True, cfg = "exec"),
        "kind": "setting",
        "resources": "light",
        "shard": "perl",
        "target_tool": True,
    },
    "perltidy": {
        "attr": attr.label(default = "//config:tool_perltidy", allow_files = True, cfg = "exec"),
        "kind": "setting",
        "resources": "light",
        "shard": "perl",
        "target_tool": True,
    },
}

SHARD_SPECS = {
    "real_perl_format": {
        "capability": "format",
        "doc": "Additive Perl format family aspect (perltidy).",
        "shard": "perl",
        "suffix": "-perl",
    },
    "real_perl_lint": {
        "capability": "lint",
        "doc": "Additive Perl lint family aspect (perlcritic).",
        "shard": "perl",
        "suffix": "-perl",
    },
}

_CFG = real_config(
    tools = TOOLS,
    shard_specs = SHARD_SPECS,
    hook_order = [],
    context_hooks = {},
    source_recognizers = [],
    harness_sources = lambda _target, _capability, direct_sources: direct_sources,
    toolchain = None,
    check_policy = False,
)

real_perl_format_aspect = real_make_aspect(_CFG, "real_perl_format")

real_perl_lint_aspect = real_make_aspect(_CFG, "real_perl_lint")

PerlShardProbeInfo = provider(
    doc = "Shard names the perl shard probe found for one target and capability.",
    fields = {
        "capability": "lint, format or typecheck",
        "shards": "sorted shard names whose tool filter keeps a resolved stage",
    },
)

real_perl_format_probe_aspect = real_make_probe_aspect(_CFG, "format", "perl", PerlShardProbeInfo)

real_perl_lint_probe_aspect = real_make_probe_aspect(_CFG, "lint", "perl", PerlShardProbeInfo)
