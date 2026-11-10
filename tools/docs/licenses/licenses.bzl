"""Renders the table of licenses of the tools rules_dx downloads."""

load("@bazel_lib//lib:write_source_files.bzl", "write_source_files")
load("@bazel_skylib//rules:write_file.bzl", "write_file")
load("//appimage/private:pins.bzl", "MKSQUASHFS_LICENSE", "MKSQUASHFS_VERSION", "RUNTIME_LICENSE", "RUNTIME_VERSION", "TOOL_LICENSE", "TOOL_VERSION")
load("//core/quality/artifacts:extension.bzl", "TOOL_ARTIFACTS", "TOOL_SOURCES")

_WRITE_GOOD_VERSION = "0.4.1"

_HEADER = """# Licenses

This page lists the license of each tool rules_dx downloads. It is generated.
To refresh it, run `bazel run //docs:update_licenses`.

Terraform is BUSL-1.1. That is not an open-source license and it restricts some uses. Read it before you enable terraform.

| Tool | Version | License |
| --- | --- | --- |
"""

def _license_names(tool, licenses):
    if not licenses:
        fail("docs/licenses.md: %s has no licence in its pin" % tool)
    return ", ".join(sorted({entry["name"]: True for entry in licenses}.keys()))

def licenses_rows():
    """Returns the sorted table rows as (tool, version, licenses) tuples."""
    rows = {}
    for artifact in TOOL_ARTIFACTS:
        rows[artifact["tool"]] = (artifact["upstream_version"], _license_names(artifact["tool"], artifact["licenses"]))
    for source in TOOL_SOURCES:
        rows[source["tool"]] = (source["version"], _license_names(source["tool"], source["licenses"]))
    rows["appimagetool"] = (TOOL_VERSION, TOOL_LICENSE)
    rows["appimage runtime"] = (RUNTIME_VERSION, RUNTIME_LICENSE)
    rows["mksquashfs"] = (MKSQUASHFS_VERSION, MKSQUASHFS_LICENSE)
    rows["write-good (vale style)"] = (_WRITE_GOOD_VERSION, "MIT")
    return [(tool, rows[tool][0], rows[tool][1]) for tool in sorted(rows.keys())]

def licenses_page():
    """Returns the full text of docs/licenses.md."""
    lines = ["| %s | %s | %s |" % row for row in licenses_rows()]
    return _HEADER + "\n".join(lines) + "\n"

def licenses_docs(name):
    """Declares the generated licenses page, its update target and its currency test."""
    write_file(
        name = name + "_generated",
        out = name + ".md",
        content = licenses_page().splitlines(),
    )
    write_source_files(
        name = name,
        files = {"licenses.md": ":" + name + "_generated"},
    )
