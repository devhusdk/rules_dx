"""Haskell real quality aspects."""

load("//quality:compile_check.bzl", "compile_check_aspect")
load("//quality:real_core.bzl", "real_config", "real_make_aspect")

TOOLS = {
    "ghc": {
        "kind": "upstream",
        "resources": "heavy",
        "shard": "haskell",
    },
    "hlint": {
        "attr": attr.label(default = "//config:tool_hlint", allow_files = True, cfg = "exec"),
        "fallback": "=<label of an hlint executable>",
        "kind": "file",
        "resources": "medium",
        "shard": "haskell",
    },
    "ormolu": {
        "attr": attr.label(default = "//config:tool_ormolu", allow_files = True, cfg = "exec"),
        "fallback": "=<label of an ormolu executable>",
        "kind": "file",
        "resources": "medium",
        "shard": "haskell",
    },
}

SHARD_SPECS = {
    "real_haskell_format": {
        "capability": "format",
        "doc": "Additive Haskell format family aspect (ormolu).",
        "shard": "haskell",
        "suffix": "-haskell",
    },
    "real_haskell_typecheck": {
        "capability": "typecheck",
        "doc": "Additive Haskell typecheck family aspect (ghc -fno-code replay of the compile action).",
        "requires": [compile_check_aspect],
        "shard": "haskell",
        "suffix": "-haskell",
    },
    "real_haskell_lint": {
        "capability": "lint",
        "doc": "Additive Haskell lint family aspect (hlint).",
        "shard": "haskell",
        "suffix": "-haskell",
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

real_haskell_format_aspect = real_make_aspect(_CFG, "real_haskell_format")

real_haskell_lint_aspect = real_make_aspect(_CFG, "real_haskell_lint")

real_haskell_typecheck_aspect = real_make_aspect(_CFG, "real_haskell_typecheck")
