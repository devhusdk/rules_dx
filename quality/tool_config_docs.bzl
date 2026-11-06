"""Renders the tool config table in docs/cli/commands/quality.md from tools.bzl."""

load("@bazel_lib//lib:write_source_files.bzl", "write_source_files")
load("@bazel_skylib//rules:write_file.bzl", "write_file")
load(":curated_defaults.bzl", "CROSS_FAMILY_TOOLS", "CURATED_DEFAULTS")
load(":tools.bzl", "TOOLS")

BEGIN = "<!-- tool-config-table:begin (generated from quality/tools.bzl; run `bazel run //docs:update_tool_configs`) -->"
END = "<!-- tool-config-table:end -->"

_HEADER = [
    "| Tool | Level | Native file(s) | Base | Tier source | Shared with |",
    "| --- | --- | --- | --- | --- | --- |",
]

def _curated():
    ids = {tool: True for tool in CROSS_FAMILY_TOOLS}
    for entry in CURATED_DEFAULTS.values():
        for tools in entry.values():
            for tool in tools:
                ids[tool] = True
    return ids

def _files(config):
    if config.files:
        return ", ".join(["`%s`" % file for file in config.files])
    return ", ".join(["`*%s`" % suffix if suffix.startswith(".") else "`%s`" % suffix for suffix in config.suffixes])

def _base(config):
    if config.extends_syntax:
        return "extends"
    if config.ext:
        return "override copy"
    return "none"

def tool_config_table():
    """Returns the generated table, one line per element."""
    curated = _curated()
    shared = {}
    for tool in TOOLS:
        if tool.config.shared and tool.config.files:
            shared.setdefault(tool.config.files[0], []).append(tool.id)
    rows = []
    for tool in sorted(TOOLS, key = lambda tool: tool.id):
        config = tool.config
        if config.kind == "none":
            continue
        others = [id for id in shared.get(config.files[0] if config.files else "", []) if id != tool.id]
        rows.append("| `%s` | %s | %s | %s | %s | %s |" % (
            tool.id,
            "curated" if tool.id in curated else "catalog",
            _files(config),
            _base(config),
            config.tier_source if config.tier_active else "",
            ", ".join(["`%s`" % id for id in others]),
        ))
    return [BEGIN, ""] + _HEADER + rows + ["", END]

def tool_config_docs(name, page):
    """Declares the update target and currency test for the config table in `page`."""
    write_file(
        name = name + "_block",
        out = name + "_block.md",
        content = tool_config_table() + [""],
    )
    native.genrule(
        name = name + "_page",
        srcs = [page, ":" + name + "_block"],
        outs = [name + "_page.md"],
        cmd = """awk -v block="$(location :%s_block)" '
/<!-- tool-config-table:begin/ { while ((getline line < block) > 0) print line; skip = 1; next }
/<!-- tool-config-table:end -->/ { skip = 0; next }
!skip { print }
' $(location %s) > $@""" % (name, page),
    )
    write_source_files(
        name = name,
        files = {page: ":" + name + "_page"},
    )
