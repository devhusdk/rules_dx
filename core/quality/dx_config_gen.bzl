"""Renders the curated tool selection that dx_config reads from curated_defaults.bzl."""

load("@bazel_lib//lib:write_source_files.bzl", "write_source_files")
load("@bazel_skylib//rules:write_file.bzl", "write_file")
load(":adapters.bzl", "REAL_CLASS_TO_FAMILY")
load(":curated_defaults.bzl", "CROSS_FAMILY_TOOLS", "CURATED_DEFAULTS")
load(":overlaps.bzl", "overlap_rows")
load(":real_aspect_labels.bzl", "REAL_ASPECT_LABELS", "REAL_PROBE_LABELS")
load(":tools.bzl", "TOOLS")

_HEADER = """// Generated from core/quality/curated_defaults.bzl and core/quality/tools.bzl.
// Run `bazel run //cli/dx_config:update_curated_defaults` to refresh it.

use crate::select::{CuratedFamily, Overlap};

"""

def _slice(ids):
    return "&[" + ", ".join(['"%s"' % id for id in ids]) + "]"

def _opt_in_families():
    families = {}
    for tool in TOOLS:
        for classes in tool.adapters.values():
            for class_id in classes:
                family = REAL_CLASS_TO_FAMILY[class_id]
                if family not in CURATED_DEFAULTS:
                    families[family] = True
    return sorted(families.keys())

def curated_rust():
    """Returns the full text of cli/dx_config/src/curated_defaults_gen.rs."""
    out = _HEADER + "pub const FAMILIES: &[CuratedFamily] = &[\n"
    for family in sorted(CURATED_DEFAULTS.keys()):
        entry = CURATED_DEFAULTS[family]
        out += "    CuratedFamily {\n        id: \"%s\",\n" % family
        for capability in ["lint", "format", "typecheck", "audit"]:
            out += "        %s: %s,\n" % (capability, _slice(entry[capability]))
        out += "    },\n"
    out += "];\n\npub const OPT_IN_FAMILIES: &[&str] = &[\n"
    out += "".join(['    "%s",\n' % id for id in _opt_in_families()])
    out += "];\n\npub const CROSS_FAMILY_TOOLS: &[&str] = &[\n"
    out += "".join(['    "%s",\n' % id for id in sorted(CROSS_FAMILY_TOOLS)])
    out += "];\n\npub const TOOLS: &[&str] = &[\n"
    out += "".join(['    "%s",\n' % id for id in sorted([tool.id for tool in TOOLS])])
    out += "];\n\npub const OVERLAPS: &[Overlap] = &[\n"
    for family, class_id, concern, first, second in overlap_rows():
        out += '    Overlap {\n        family: "%s",\n        class: "%s",\n        concern: "%s",\n        tools: [("%s", "%s"), ("%s", "%s")],\n    },\n' % (family, class_id, concern, first[0], first[1], second[0], second[1])
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

_REGISTRY_HEADER = """// Generated from core/quality/real_aspect_labels.bzl.
// Run `bazel run //cli/plan:update_registry_gen` to refresh it.

"""

def _label_const(name, labels):
    return "pub const %s: &[&str] = &[\n%s];\n" % (name, "".join(['    "%s",\n' % label for label in labels]))

def registry_rust():
    """Returns the full text of cli/plan/src/plan/registry_gen.rs."""
    consts = []
    for suffix, table in [("ASPECTS", REAL_ASPECT_LABELS), ("PROBES", REAL_PROBE_LABELS)]:
        for command in sorted(table.keys()):
            consts.append(_label_const("%s_%s" % (command.upper(), suffix), table[command]))
    return _REGISTRY_HEADER + "\n".join(consts)

def plan_registry_gen(name):
    """Declares the generated aspect label table, its update target and its currency test."""
    write_file(
        name = name + "_generated",
        out = name + ".rs",
        content = registry_rust().splitlines() + [""],
    )
    write_source_files(
        name = name,
        files = {"src/plan/registry_gen.rs": ":" + name + "_generated"},
    )
