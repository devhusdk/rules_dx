"""Nix real quality aspects."""

load("//core/quality:real_core.bzl", "real_config", "real_make_aspect")

TOOLS = {
    "deadnix": {
        "attr": attr.label(default = "//rust/private/quality/tools:deadnix_tool", cfg = "exec", executable = True),
        "kind": "executable",
        "resources": "light",
        "shard": "nix",
    },
    "nixfmt": {
        "attr": attr.label(default = "//config:tool_nixfmt", allow_files = True, cfg = "exec"),
        "fallback": "=<label of a nixfmt executable>",
        "kind": "file",
        "resources": "light",
        "shard": "nix",
    },
}

SHARD_SPECS = {
    "real_nix_format": {
        "capability": "format",
        "doc": "Additive Nix format family aspect (nixfmt).",
        "shard": "nix",
        "suffix": "-nix",
    },
    "real_nix_lint": {
        "capability": "lint",
        "doc": "Additive Nix lint family aspect (deadnix).",
        "shard": "nix",
        "suffix": "-nix",
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

real_nix_format_aspect = real_make_aspect(_CFG, "real_nix_format")

real_nix_lint_aspect = real_make_aspect(_CFG, "real_nix_lint")
