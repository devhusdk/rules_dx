"""Renders the curated tool selection that dx_config reads from curated_defaults.bzl."""

load("@bazel_lib//lib:write_source_files.bzl", "write_source_files")
load("@bazel_skylib//rules:write_file.bzl", "write_file")
load(":curated_defaults.bzl", "CROSS_FAMILY_TOOLS", "CURATED_DEFAULTS")
load(":tools.bzl", "TOOLS")

_HEADER = """// Generated from quality/curated_defaults.bzl and quality/tools.bzl.
// Run `bazel run //cli/dx_config:update_curated_defaults` to refresh it.

use crate::select::CuratedFamily;

"""

def _slice(ids):
    return "&[" + ", ".join(['"%s"' % id for id in ids]) + "]"

def curated_rust():
    """Returns the full text of cli/dx_config/src/curated_defaults_gen.rs."""
    out = _HEADER + "pub const FAMILIES: &[CuratedFamily] = &[\n"
    for family in sorted(CURATED_DEFAULTS.keys()):
        entry = CURATED_DEFAULTS[family]
        out += "    CuratedFamily {\n        id: \"%s\",\n" % family
        for capability in ["lint", "format", "typecheck", "audit"]:
            out += "        %s: %s,\n" % (capability, _slice(entry[capability]))
        out += "    },\n"
    out += "];\n\npub const CROSS_FAMILY_TOOLS: &[&str] = &[\n"
    out += "".join(['    "%s",\n' % id for id in sorted(CROSS_FAMILY_TOOLS)])
    out += "];\n\npub const TOOLS: &[&str] = &[\n"
    out += "".join(['    "%s",\n' % id for id in sorted([tool.id for tool in TOOLS])])
    return out + "];\n"

def dx_config_gen(name):
    """Declares the generated selection table, its update target and its currency test."""
    write_file(
        name = name + "_generated",
        out = name + ".rs",
        content = curated_rust().splitlines() + [""],
    )
    write_source_files(
        name = name,
        files = {"src/curated_defaults_gen.rs": ":" + name + "_generated"},
    )
