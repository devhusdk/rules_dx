"""Renders the Rust list of quality tool ids from tools.bzl."""

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
    )
