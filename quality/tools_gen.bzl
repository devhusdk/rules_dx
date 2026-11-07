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
        additional_update_targets = ["//cli/generate:update_tools", "//quality/adapter:update_severity"],
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
    pub references: &'static [&'static str],
    pub kind: &'static str,
    pub tier_active: bool,
    pub tier_base: bool,
    pub stage_native: bool,
    pub tier_source: &'static str,
    pub family: &'static str,
    pub extensions: &'static [&'static str],
    pub init_order: u8,
    pub rule_keys: &'static [&'static str],
    pub standard: &'static str,
    pub strict: &'static str,
    pub suppressions: &'static [&'static str],
    pub suppression_extensions: &'static [&'static str],
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
    if len(line) <= 101 or value.startswith('"'):
        return line
    return "        %s:\n            %s,\n" % (name, value)

def _bool(value):
    return "true" if value else "false"

def _tier(tool, name):
    config = tool.config
    ext = config.ext
    if not ext and config.kind == "flags":
        ext = "txt"
    if not ext:
        return '""'
    path = "../../../quality/tiers/%s/%s.%s" % (config.tier_dir or tool.id, name, ext)
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
        ("references", _strs(config.references)),
        ("kind", _str(config.kind)),
        ("tier_active", _bool(config.tier_active)),
        ("tier_base", _bool(config.tier_base)),
        ("stage_native", _bool(config.stage_native)),
        ("tier_source", _str(config.tier_source)),
        ("family", _str(config.family)),
        ("extensions", _strs(config.extensions)),
        ("init_order", str(config.init_order)),
        ("rule_keys", _strs(config.rule_keys)),
        ("standard", _tier(tool, "standard")),
        ("strict", _tier(tool, "strict")),
        ("suppressions", _strs(config.suppressions)),
        ("suppression_extensions", _strs(config.suppression_extensions)),
    ]
    return "    ToolConfig {\n" + "".join([_field(name, value) for name, value in fields]) + "    },\n"

def config_rust():
    """Returns the full text of cli/generate/src/tools_gen.rs."""
    tools = sorted([tool for tool in TOOLS if tool.config.kind != "none"], key = lambda tool: (tool.config.init_order == 0, tool.config.init_order, tool.id))
    return _CONFIG_HEADER + "".join([_config_row(tool) for tool in tools]) + "];\n"

def config_tools_gen(name, visibility):
    """Declares the generated dx tool-config table and its update target."""
    write_file(
        name = name + "_generated",
        out = name + ".rs",
        content = config_rust().splitlines() + [""],
    )
    write_source_files(
        name = name,
        files = {"src/tools_gen.rs": ":" + name + "_generated"},
        visibility = visibility,
    )

_SEVERITY_HEADER = """// Generated from quality/tools.bzl. Run `bazel run //quality:update_tools` to refresh it.

pub const TOOL_SEVERITIES: &[(&str, crate::severity::ToolSeverityMap)] = &[
"""

def severity_rust():
    """Returns the full text of quality/adapter/src/severity_gen.rs."""
    out = _SEVERITY_HEADER
    mapped = [tool for tool in TOOLS if tool.severity.default != None or tool.severity.categories]
    if not mapped:
        return out.rstrip("\n") + "];\n"
    for tool in sorted(TOOLS, key = lambda tool: tool.id):
        severity = tool.severity
        if severity.default == None and not severity.categories:
            continue
        default = "None" if severity.default == None else "Some(%s)" % _level(severity.default)
        out += "    (\n        %s,\n        crate::severity::ToolSeverityMap {\n            default: %s,\n" % (_str(tool.id), default)
        if not severity.categories:
            out += "            categories: &[],\n        },\n    ),\n"
            continue
        keys = sorted(severity.categories.keys())
        if len(keys) == 1:
            out += "            categories: &[(%s, %s)],\n        },\n    ),\n" % (_str(keys[0]), _level(severity.categories[keys[0]]))
            continue
        out += "            categories: &[\n"
        for key in keys:
            pair = "%s, %s" % (_str(key), _level(severity.categories[key]))
            if len(pair) > 60:
                out += "                (\n                    %s,\n                ),\n" % pair.replace(", crate::", ",\n                    crate::")
            else:
                out += "                (%s),\n" % pair
        out += "            ],\n        },\n    ),\n"
    return out + "];\n"

def _level(level):
    return {"error": "crate::ToolSeverity::Error", "info": "crate::ToolSeverity::Info", "warning": "crate::ToolSeverity::Warning"}[level]

def severity_gen(name):
    """Declares the generated severity table and its update target."""
    write_file(
        name = name + "_generated",
        out = name + ".rs",
        content = severity_rust().splitlines() + [""],
    )
    write_source_files(
        name = name,
        files = {"src/severity_gen.rs": ":" + name + "_generated"},
        visibility = ["//quality/runner:__pkg__"],
    )
