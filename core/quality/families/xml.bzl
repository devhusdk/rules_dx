"""Xml real quality aspects."""

load("//core/quality:real_core.bzl", "real_config", "real_make_aspect")

TOOLS = {
    "xmllint": {
        "attr": attr.label(default = "//core/quality/tools/xml:xmllint", cfg = "exec", executable = True),
        "kind": "executable",
        "resources": "light",
        "shard": "xml",
    },
}

SHARD_SPECS = {
    "real_xml_lint": {
        "capability": "lint",
        "doc": "Additive XML lint family aspect (xmllint, built from the BCR libxml2 module).",
        "shard": "xml",
        "suffix": "-xml",
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

real_xml_lint_aspect = real_make_aspect(_CFG, "real_xml_lint")
