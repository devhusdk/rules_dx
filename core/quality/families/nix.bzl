"""Nix real quality aspects."""

load("//core/quality:real_core.bzl", "real_config", "real_make_aspect")

visibility(["//core/..."])

TOOLS = {
    "deadnix": {
        "attr": attr.label(default = "//config:tool_deadnix", cfg = "exec", executable = True),
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
    },
    "real_nix_lint": {
        "capability": "lint",
        "doc": "Additive Nix lint family aspect (deadnix).",
        "shard": "nix",
    },
}

HOOK_ORDER = []

HOOKS = {}

_CFG = real_config(
    tools = TOOLS,
    shard_specs = SHARD_SPECS,
)

real_nix_format_aspect = real_make_aspect(_CFG, "real_nix_format")

real_nix_lint_aspect = real_make_aspect(_CFG, "real_nix_lint")
