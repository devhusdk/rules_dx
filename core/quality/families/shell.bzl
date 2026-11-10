"""Shell real quality aspects."""

load("//core/quality:real_core.bzl", "real_config", "real_make_aspect")

TOOLS = {
    "shellcheck": {
        "attr": attr.label(default = "@dx_tools//:shellcheck", allow_single_file = True, cfg = "exec"),
        "kind": "file",
        "resources": "light",
        "shard": "shell",
    },
    "shfmt": {
        "attr": attr.label(default = "@dx_tools//:shfmt", allow_single_file = True, cfg = "exec"),
        "kind": "file",
        "resources": "light",
        "shard": "shell",
    },
}

SHARD_SPECS = {
    "real_shell_lint": {
        "capability": "lint",
        "doc": "Additive shell lint family aspect (ShellCheck).",
        "shard": "shell",
    },
    "real_shell_format": {
        "capability": "format",
        "doc": "Additive shell format family aspect (shfmt).",
        "shard": "shell",
    },
}

_CFG = real_config(
    tools = TOOLS,
    shard_specs = SHARD_SPECS,
)

real_shell_format_aspect = real_make_aspect(_CFG, "real_shell_format")

real_shell_lint_aspect = real_make_aspect(_CFG, "real_shell_lint")
