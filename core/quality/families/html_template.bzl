"""Html template real quality aspects."""

load("//core/quality:real_core.bzl", "real_config", "real_make_aspect")

TOOLS = {
    "djlint": {
        "attr": attr.label(default = "@dx_prebuilt//:python_tool_djlint", cfg = "exec", executable = True),
        "kind": "executable",
        "resources": "medium",
        "runfiles_env": True,
        "shard": "html_template",
    },
}

SHARD_SPECS = {
    "real_html_template_format": {
        "capability": "format",
        "doc": "Additive HTML template format family aspect (djLint).",
        "shard": "html_template",
        "suffix": "-html-template",
    },
    "real_html_template_lint": {
        "capability": "lint",
        "doc": "Additive HTML template lint family aspect (djLint).",
        "shard": "html_template",
        "suffix": "-html-template",
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

real_html_template_format_aspect = real_make_aspect(_CFG, "real_html_template_format")

real_html_template_lint_aspect = real_make_aspect(_CFG, "real_html_template_lint")
