"""Renders the Rust tables generated from tools.bzl: the tool ids and the dx config metadata."""

load("@bazel_lib//lib:write_source_files.bzl", "write_source_files")
load("@bazel_skylib//rules:write_file.bzl", "write_file")
load(":tools.bzl", "TOOLS")

_HEADER = """// Generated from quality/tools.bzl. Run `bazel run //quality:update_tools` to refresh it.

pub const REAL_TOOLS: &[&str] = &[
"""

def tools_rust():
    """Returns the full text of quality/runner/src/real/tools_gen.rs."""
    ids = sorted([tool.id for tool in TOOLS])
    return _HEADER + "".join(['    "%s",\n' % id for id in ids]) + "];\n"

def tools_gen(name):
    """Declares the generated tool list, its update target and its currency test."""
    write_file(
        name = name + "_generated",
        out = name + ".rs",
        content = tools_rust().splitlines() + [""],
    )
    write_source_files(
        name = name,
        files = {"src/real/tools_gen.rs": ":" + name + "_generated"},
        additional_update_targets = ["//cli/adopt:update_tools"],
    )

_CONFIG_HEADER = """// Generated from quality/tools.bzl. Run `bazel run //quality:update_tools` to refresh it.

#[allow(dead_code)]
pub struct ToolConfig {
    pub id: &'static str,
    pub suffixes: &'static [&'static str],
    pub files: &'static [&'static str],
    pub ext: &'static str,
    pub extends_syntax: &'static str,
    pub shared: bool,
    pub tier_dir: &'static str,
    pub comment: &'static str,
    pub pyproject_table: &'static str,
    pub extras: &'static [&'static str],
    pub kind: &'static str,
    pub tier_active: bool,
    pub stage_native: bool,
    pub tier_source: &'static str,
    pub family: &'static str,
    pub extensions: &'static [&'static str],
    pub init_order: u8,
    pub standard: &'static str,
    pub strict: &'static str,
}

pub const TOOL_CONFIGS: &[ToolConfig] = &[
"""

def _str(value):
    return json.encode(value)

def _strs(values):
    return values

def _field(name, value):
    # Lays a field out the way rustfmt does, so the generated file is a fixpoint.
    if type(value) == "list":
        line = "        %s: &[%s],\n" % (name, ", ".join([_str(item) for item in value]))
        if len(line) <= 101:
            return line
        return "        %s: &[\n%s        ],\n" % (name, "".join(["            %s,\n" % _str(item) for item in value]))
    line = "        %s: %s,\n" % (name, value)
    if len(line) <= 101:
        return line
    return "        %s:\n            %s,\n" % (name, value)

def _bool(value):
    return "true" if value else "false"

def _tier(tool, name):
    config = tool.config
    if not config.ext:
        return '""'
    path = "../../../quality/tiers/%s/%s.%s" % (config.tier_dir or tool.id, name, config.ext)
    return 'include_str!("%s")' % path

def _config_row(tool):
    config = tool.config
    fields = [
        ("id", _str(tool.id)),
        ("suffixes", _strs(config.suffixes)),
        ("files", _strs(config.files)),
        ("ext", _str(config.ext)),
        ("extends_syntax", _str(config.extends_syntax)),
        ("shared", _bool(config.shared)),
        ("tier_dir", _str(config.tier_dir)),
        ("comment", _str(config.comment)),
        ("pyproject_table", _str(config.pyproject_table)),
        ("extras", _strs(config.extras)),
        ("kind", _str(config.kind)),
        ("tier_active", _bool(config.tier_active)),
        ("stage_native", _bool(config.stage_native)),
        ("tier_source", _str(config.tier_source)),
        ("family", _str(config.family)),
        ("extensions", _strs(config.extensions)),
        ("init_order", str(config.init_order)),
        ("standard", _tier(tool, "standard")),
        ("strict", _tier(tool, "strict")),
    ]
    return "    ToolConfig {\n" + "".join([_field(name, value) for name, value in fields]) + "    },\n"

def adopt_rust():
    """Returns the full text of cli/adopt/src/tools_gen.rs."""
    tools = sorted([tool for tool in TOOLS if tool.config.kind != "none"], key = lambda tool: (tool.config.init_order == 0, tool.config.init_order, tool.id))
    return _CONFIG_HEADER + "".join([_config_row(tool) for tool in tools]) + "];\n"

def adopt_tools_gen(name, visibility):
    """Declares the generated dx tool-config table and its update target."""
    write_file(
        name = name + "_generated",
        out = name + ".rs",
        content = adopt_rust().splitlines() + [""],
    )
    write_source_files(
        name = name,
        files = {"src/tools_gen.rs": ":" + name + "_generated"},
        visibility = visibility,
    )
