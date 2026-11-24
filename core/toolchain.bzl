"""Pinned-toolchain factory: one repository per host cell, plus the checks every pin table shares."""

load("//core/private/testing:defs.bzl", "expect_equal", "expect_true")
load(":ghcr.bzl", "ghcr_headers")
load(":host.bzl", "host_cell")
load(":repo.bzl", "gem_extract")

visibility(["//cc/...", "//core/...", "//cuda/...", "//d/...", "//elixir/...", "//erlang/...", "//fortran/...", "//haskell/...", "//lua/...", "//msix/...", "//ocaml/...", "//powershell/...", "//swift/..."])

_HEX = "0123456789abcdef"

_WALK_LIMIT = 1000000

_HOST_CELLS = [
    "linux_arm64",
    "linux_x86_64",
    "macos_arm64",
    "macos_x86_64",
    "windows_arm64",
    "windows_x86_64",
]

def _fetch(rctx, tool, pin, build):
    binary = pin["binary"]
    if "stub" in pin:
        rctx.file(binary, pin["stub"], executable = True)
    elif pin.get("type") == "gem":
        gem_extract(
            rctx,
            url = pin["urls"],
            sha256 = pin["sha256"],
            canonical_id = "dx-gem:" + pin["urls"][0],
            output = ".",
            required = [binary],
        )
    else:
        archive_type = pin.get("type", "")
        strip_prefix = pin.get("strip_prefix", "")
        if not pin.get("ghcr_scope") or not rctx.download_and_extract(url = pin["urls"], sha256 = pin["sha256"], type = archive_type, stripPrefix = strip_prefix, allow_fail = True).success:
            headers = ghcr_headers(rctx, pin["ghcr_scope"]) if pin.get("ghcr_scope") else {}
            rctx.download_and_extract(url = pin["urls"], sha256 = pin["sha256"], type = archive_type, stripPrefix = strip_prefix, headers = headers)
        if not rctx.path(binary).exists:
            fail("{} archive {} has no {}".format(tool, pin["urls"][0], binary))
        if pin.get("executable"):
            rctx.file(binary, rctx.read(binary), executable = True)
    rctx.file("REPO.bazel", "")
    rctx.file("BUILD.bazel", build.replace("{binary}", binary))
    return rctx.repo_metadata(reproducible = True)

_EXPORT_LINE = 'exports_files(["{binary}"])'

_UNPINNED_GENRULE = """genrule(
    name = "unpinned",
    outs = [{binary}],
    cmd = {cmd},
)"""

def _unpinned(rctx, tool, cell, cells, build):
    if _EXPORT_LINE not in build:
        fail("{}: the BUILD file has no {} line".format(tool, _EXPORT_LINE))
    binary = cells[sorted(cells)[0]]["binary"]
    message = "no pinned {} archive for {}".format(tool, cell)
    rctx.file("REPO.bazel", "")
    rctx.file("BUILD.bazel", build.replace(_EXPORT_LINE, _UNPINNED_GENRULE.format(
        binary = repr(binary),
        cmd = repr("echo '{}' >&2; exit 1".format(message)),
    )))
    return rctx.repo_metadata(reproducible = True)

def _host_archive_impl(rctx):
    cell = rctx.attr.aliases.get(host_cell(rctx), host_cell(rctx))
    cells = json.decode(rctx.attr.cells)
    if cell not in cells:
        return _unpinned(rctx, rctx.attr.tool, cell, cells, rctx.attr.build)
    return _fetch(rctx, rctx.attr.tool, cells[cell], rctx.attr.build)

host_archive_rule = repository_rule(
    implementation = _host_archive_impl,
    doc = "Downloads the pinned archive of the host cell and writes its BUILD file; on a host without a pin, building the binary fails.",
    attrs = {
        "aliases": attr.string_dict(doc = "Host cell to the pinned cell that serves it."),
        "build": attr.string(mandatory = True, doc = "BUILD file content; {binary} is replaced by the cell's binary path."),
        "cells": attr.string(mandatory = True, doc = "JSON object of cell to {urls, sha256, binary, strip_prefix, type, ghcr_scope}."),
        "tool": attr.string(mandatory = True, doc = "Tool name used in failure messages."),
    },
)

def _cell_archive_impl(rctx):
    return _fetch(rctx, rctx.attr.tool, json.decode(rctx.attr.pin), rctx.attr.build)

cell_archive_rule = repository_rule(
    implementation = _cell_archive_impl,
    doc = "Downloads the pinned archive of one cell and writes its BUILD file.",
    attrs = {
        "build": attr.string(mandatory = True, doc = "BUILD file content; {binary} is replaced by the cell's binary path."),
        "pin": attr.string(mandatory = True, doc = "JSON object of the cell's urls, sha256, binary and optional strip_prefix, type and ghcr_scope."),
        "tool": attr.string(mandatory = True, doc = "Tool name used in failure messages."),
    },
)

def pin_checks(cells, version, archive_attr = "archive"):
    """Returns the checks every archive pin table shares."""
    pins = cells.values()
    return [
        expect_true("digests are sha256", all([is_sha256(p.sha256) for p in pins])),
        expect_true("urls name the version and archive", all([version in p.urls[0] and p.urls[0].endswith(getattr(p, archive_attr)) for p in pins])),
        expect_equal("cells are host cells", [c for c in cells if c not in _HOST_CELLS], []),
    ]

def is_sha256(value):
    """Returns whether value is a sha256 digest of 64 lowercase hex characters."""
    return type(value) == "string" and len(value) == 64 and all([c in _HEX for c in value.elems()])

def digest_problems(name, value):
    """Returns the sorted "<path>: <problem>" strings for every sha256 and urls nested in value."""
    problems = []
    stack = [(name, value)]
    for _ in range(_WALK_LIMIT):
        if not stack:
            return sorted(problems)
        path, node = stack.pop()
        if type(node) == "list":
            stack.extend([("{}[{}]".format(path, i), item) for i, item in enumerate(node)])
        elif type(node) == "dict":
            if "sha256" in node:
                digest = node["sha256"]
                if not (is_sha256(digest) or (type(digest) == "dict" and len(digest) > 0 and all([is_sha256(d) for d in digest.values()]))):
                    problems.append(path + ": sha256 is not 64 lowercase hex characters")
            if "urls" in node:
                urls = node["urls"]
                if type(urls) != "list" or len(urls) == 0 or not all([type(url) == "string" and url.startswith("https://") for url in urls]):
                    problems.append(path + ": urls is not a non-empty list of https urls")
            stack.extend([(path + "." + key, item) for key, item in node.items()])
    fail("{}: more than {} nested values".format(name, _WALK_LIMIT))

def toolchain_target(name, toolchain, toolchain_type, exec_compatible_with = None, target_compatible_with = None, target_settings = None, visibility = None):
    """Returns one toolchain() target; attributes left as None are omitted."""
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
    """Returns the checks every toolchain hub BUILD file shares."""
    return [
        expect_true("hub loads nothing", "load(" not in hub),
        expect_true("hub names the toolchain type", toolchain_type in hub),
        expect_equal("hub toolchain count", hub.count("toolchain("), count),
    ] + [expect_true("hub does not name " + repo, repo not in hub) for repo in forbidden]

_OS_CONSTRAINTS = {"linux": "@platforms//os:linux", "macos": "@platforms//os:macos", "windows": "@platforms//os:windows"}

_CPU_CONSTRAINTS = {"arm64": "@platforms//cpu:aarch64", "x86_64": "@platforms//cpu:x86_64"}

def cell_repo(name, cell):
    """Returns the name of the repository holding one cell of a pin."""
    return name + "__" + cell

def pins_hub(name, cells, toolchain_type, compatible_with = "exec", exec_aliases = [], os_constraints = {}, exec_toolchain_type = None, variants = {}, companion_types = {}, alias_defaults = {}, repository = None, target = "toolchain", target_settings = None, cell_constraints = {}):
    """Returns the hub BUILD file of a pin: one toolchain() per cell and an alias per exec_aliases target selecting the exec cell."""
    constraints = {
        cell: [_OS_CONSTRAINTS[cell.partition("_")[0]], _CPU_CONSTRAINTS[cell.partition("_")[2]]] + os_constraints.get(cell.partition("_")[0], [])
        for cell in cells
        if cell not in cell_constraints
    } | {cell: cell_constraints[cell].exec_compatible_with for cell in cells if cell in cell_constraints}
    target_constraints = constraints | {cell: cell_constraints[cell].target_compatible_with for cell in cells if cell in cell_constraints}
    sides = ["exec", "target"] if compatible_with == "both" else [compatible_with]
    parts = []
    for cell in sorted(cells):
        repo = repository or cell_repo(name, cell)
        for variant in sorted(variants) or [""]:
            compatible = {
                side + "_compatible_with": (constraints if side == "exec" else target_constraints)[cell] + (getattr(variants[variant], side + "_compatible_with") if variant else [])
                for side in sides
            }
            toolchain = cell_repo(name, cell) + ("_" + variant if variant else "")
            cell_target = variant or target
            parts.append(toolchain_target(
                name = toolchain,
                toolchain = "@{}//:{}".format(repo, cell_target),
                toolchain_type = toolchain_type,
                target_settings = target_settings,
                visibility = ["//visibility:public"],
                **compatible
            ))
            parts.extend([
                toolchain_target(
                    name = toolchain + "_" + suffix,
                    toolchain = "@{}//:{}_{}".format(repo, cell_target, suffix),
                    toolchain_type = companion_types[suffix],
                    target_settings = target_settings,
                    visibility = ["//visibility:public"],
                    **compatible
                )
                for suffix in sorted(companion_types)
            ])
    if exec_toolchain_type:
        parts.extend([
            toolchain_target(
                name = cell_repo(name, cell) + "_exec",
                exec_compatible_with = constraints[cell],
                toolchain = "@{}//:{}".format(repository or cell_repo(name, cell), target),
                toolchain_type = exec_toolchain_type,
                target_settings = target_settings,
                visibility = ["//visibility:public"],
            )
            for cell in sorted(cells)
        ])
    if exec_aliases:
        parts.extend([
            'config_setting(\n    name = "{}_exec",\n    constraint_values = {},\n)\n'.format(cell, repr(constraints[cell]))
            for cell in sorted(cells)
        ])
        aliases = exec_aliases if type(exec_aliases) == "dict" else {target: "//:" + target for target in exec_aliases}

        # An alias without a default is incompatible off the cells, so Bazel skips its dependents there.
        incompatible = repr({":{}_exec".format(cell): [] for cell in sorted(cells)} | {"//conditions:default": ["@platforms//:incompatible"]})
        parts.extend([
            'alias(\n    name = "{}",\n    actual = select({}),\n{}    visibility = ["//visibility:public"],\n)\n'.format(
                alias,
                repr(
                    {":{}_exec".format(cell): "@{}{}".format(repository or cell_repo(name, cell), aliases[alias]) for cell in sorted(cells)} |
                    ({"//conditions:default": alias_defaults[alias]} if alias in alias_defaults else {}),
                ),
                "" if alias in alias_defaults else "    target_compatible_with = select({}),\n".format(incompatible),
            )
            for alias in aliases
        ])
    return "\n".join(parts)
