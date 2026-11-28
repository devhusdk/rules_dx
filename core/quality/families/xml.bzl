"""Xml real quality aspects."""

load("//core/quality:real_core.bzl", "real_config", "real_make_aspect")

visibility(["//core/..."])

TOOLS = {
    "xmllint": {
        "attr": attr.label(default = "//config:tool_xmllint", allow_single_file = True, cfg = "exec"),
        "kind": "file",
        "resources": "light",
        "shard": "xml",
    },
}

SHARD_SPECS = {
    "real_xml_lint": {
        "capability": "lint",
        "doc": "Additive XML lint family aspect (xmllint).",
        "shard": "xml",
    },
}

HOOK_ORDER = []

HOOKS = {}

_CFG = real_config(
    tools = TOOLS,
    shard_specs = SHARD_SPECS,
)

real_xml_lint_aspect = real_make_aspect(_CFG, "real_xml_lint")
