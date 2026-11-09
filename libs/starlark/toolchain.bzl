"""Pinned-toolchain factory: one repository per host cell, plus the checks every pin table shares."""

load(":defs.bzl", "expect_equal", "expect_true")
load(":host.bzl", "host_cell")

_HOST_CELLS = [
    "linux_arm64",
    "linux_x86_64",
    "macos_arm64",
    "macos_x86_64",
    "windows_arm64",
    "windows_x86_64",
]

def _host_archive_impl(rctx):
    cell = rctx.attr.aliases.get(host_cell(rctx), host_cell(rctx))
    cells = json.decode(rctx.attr.cells)
    pin = cells.get(cell)
    if pin == None:
        fail("no pinned {} archive for {}".format(rctx.attr.tool, cell))
    rctx.download_and_extract(url = pin["urls"], sha256 = pin["sha256"], stripPrefix = pin.get("strip_prefix", ""))
    if not rctx.path(pin["binary"]).exists:
        fail("{} archive {} has no {}".format(rctx.attr.tool, pin["urls"][0], pin["binary"]))
    rctx.file("REPO.bazel", "")
    rctx.file("BUILD.bazel", rctx.attr.build.replace("{binary}", pin["binary"]))
    return rctx.repo_metadata(reproducible = True)

_host_archive = repository_rule(
    implementation = _host_archive_impl,
    doc = "Downloads the pinned archive of the host cell and writes its BUILD file.",
    attrs = {
        "aliases": attr.string_dict(doc = "Host cell to the pinned cell that serves it."),
        "build": attr.string(mandatory = True, doc = "BUILD file content; {binary} is replaced by the cell's binary path."),
        "cells": attr.string(mandatory = True, doc = "JSON object of cell to {urls, sha256, binary, strip_prefix}."),
        "tool": attr.string(mandatory = True, doc = "Tool name used in failure messages."),
    },
)

def host_archive(name, tool, cells, build, aliases = {}):
    """Declares a repository holding the pinned archive of the host cell.

    Args:
        name: Repository name.
        tool: Tool name used in failure messages.
        cells: Dict of cell to a struct with urls, sha256 and binary, and optionally strip_prefix.
        build: BUILD file content; {binary} is replaced by the cell's binary path.
        aliases: Dict of host cell to the pinned cell that serves it.
    """
    _host_archive(
        name = name,
        aliases = aliases,
        build = build,
        cells = json.encode({cell: {"binary": pin.binary, "sha256": pin.sha256, "strip_prefix": getattr(pin, "strip_prefix", ""), "urls": pin.urls} for cell, pin in cells.items()}),
        tool = tool,
    )

def pin_checks(cells, version, archive_attr = "archive"):
    """Returns the checks every archive pin table shares.

    Args:
        cells: Dict of cell to a struct with sha256, urls and the archive file name.
        version: Version string every URL must name.
        archive_attr: Struct field holding the archive file name each first URL must end with.
    """
    pins = cells.values()
    return [
        expect_true("digests are sha256", all([len(p.sha256) == 64 for p in pins])),
        expect_true("urls name the version and archive", all([version in p.urls[0] and p.urls[0].endswith(getattr(p, archive_attr)) for p in pins])),
        expect_equal("cells are host cells", [c for c in cells if c not in _HOST_CELLS], []),
    ]
