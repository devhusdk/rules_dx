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

def toolchain_target(name, toolchain, toolchain_type, exec_compatible_with = None, target_compatible_with = None, target_settings = None, visibility = None):
    """Returns one toolchain() target; attributes left as None are omitted.

    Args:
        name: Target name.
        toolchain: Label string of the toolchain implementation.
        toolchain_type: Label string of the toolchain type.
        exec_compatible_with: List of constraint label strings, or None.
        target_compatible_with: List of constraint label strings, or None.
        target_settings: List of config_setting label strings, or None.
        visibility: List of visibility label strings, or None.
    """
    lines = ["toolchain(", "    name = {},".format(repr(name))]
    for attr, value in [
        ("exec_compatible_with", exec_compatible_with),
        ("target_compatible_with", target_compatible_with),
        ("target_settings", target_settings),
    ]:
        if value != None:
            lines.append("    {} = {},".format(attr, repr(list(value))))
    lines.append("    toolchain = {},".format(repr(toolchain)))
    lines.append("    toolchain_type = {},".format(repr(toolchain_type)))
    if visibility != None:
        lines.append("    visibility = {},".format(repr(list(visibility))))
    lines.append(")")
    return "\n".join(lines) + "\n"

def hub_checks(hub, toolchain_type, count, forbidden = []):
    """Returns the checks every toolchain hub BUILD file shares.

    Args:
        hub: Rendered hub BUILD file content.
        toolchain_type: Label string the hub must name.
        count: Number of toolchain() targets the hub must hold.
        forbidden: Repository names the hub must not mention.
    """
    return [
        expect_true("hub loads nothing", "load(" not in hub),
        expect_true("hub names the toolchain type", toolchain_type in hub),
        expect_equal("hub toolchain count", hub.count("toolchain("), count),
    ] + [expect_true("hub does not name " + repo, repo not in hub) for repo in forbidden]
