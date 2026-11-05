"""SQL real quality aspects."""

load("//quality:real_core.bzl", "real_config", "real_make_aspect")

TOOLS = {
    "sqlfluff": {
        "attr": attr.label(default = "//quality/tools/python:sqlfluff", cfg = "exec", executable = True),
        "kind": "executable",
        "resources": "medium",
        "runfiles_env": True,
        "shard": "sql",
    },
}

SHARD_SPECS = {
    "real_sql_format": {
        "capability": "format",
        "doc": "Additive SQL format family aspect (sqlfluff).",
        "shard": "sql",
        "suffix": "-sql",
    },
    "real_sql_lint": {
        "capability": "lint",
        "doc": "Additive SQL lint family aspect (sqlfluff).",
        "shard": "sql",
        "suffix": "-sql",
    },
}

HOOK_ORDER = []

HOOKS = {}

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

real_sql_format_aspect = real_make_aspect(_CFG, "real_sql_format")

real_sql_lint_aspect = real_make_aspect(_CFG, "real_sql_lint")
