"""Pinned-toolchain factory: one repository per host cell, plus the checks every pin table shares."""

load(":defs.bzl", "expect_equal", "expect_true")
load(":host.bzl", "host_cell")
load(":repo.bzl", "gem_extract")

visibility(["//cc/...", "//core/...", "//d/...", "//elixir/...", "//erlang/...", "//fortran/...", "//haskell/...", "//lua/...", "//msix/...", "//ocaml/...", "//powershell/...", "//swift/..."])

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
        args = dict(url = pin["urls"], sha256 = pin["sha256"], type = pin.get("type", ""), stripPrefix = pin.get("strip_prefix", ""))
        if not pin.get("ghcr_scope") or not rctx.download_and_extract(allow_fail = True, **args).success:
            headers = {}
            if pin.get("ghcr_scope"):
                rctx.download(url = "https://ghcr.io/token?service=ghcr.io&scope=" + pin["ghcr_scope"], output = "token.json")
                headers["Authorization"] = ["Bearer " + json.decode(rctx.read("token.json"))["token"]]
                rctx.delete("token.json")
            rctx.download_and_extract(headers = headers, **args)
        if not rctx.path(binary).exists:
            fail("{} archive {} has no {}".format(tool, pin["urls"][0], binary))
        if pin.get("executable"):
            rctx.file(binary, rctx.read(binary), executable = True)
    rctx.file("REPO.bazel", "")
    rctx.file("BUILD.bazel", build.replace("{binary}", binary))
    return rctx.repo_metadata(reproducible = True)

def _host_archive_impl(rctx):
    cell = rctx.attr.aliases.get(host_cell(rctx), host_cell(rctx))
    pin = json.decode(rctx.attr.cells).get(cell)
    if pin == None:
        fail("no pinned {} archive for {}".format(rctx.attr.tool, cell))
    return _fetch(rctx, rctx.attr.tool, pin, rctx.attr.build)

host_archive_rule = repository_rule(
    implementation = _host_archive_impl,
    doc = "Downloads the pinned archive of the host cell and writes its BUILD file.",
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
        expect_true("digests are sha256", all([len(p.sha256) == 64 for p in pins])),
        expect_true("urls name the version and archive", all([version in p.urls[0] and p.urls[0].endswith(getattr(p, archive_attr)) for p in pins])),
        expect_equal("cells are host cells", [c for c in cells if c not in _HOST_CELLS], []),
    ]

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

def pins_hub(name, cells, toolchain_type, compatible_with = "exec", exec_aliases = [], os_constraints = {}, exec_toolchain_type = None, variants = {}, companion_types = {}, alias_defaults = {}, repository = None):
    """Returns the hub BUILD file of a pin: one toolchain() per cell and an alias per exec_aliases target selecting the exec cell.

    os_constraints maps an OS to constraints every cell of that OS adds after its OS and CPU.
    exec_toolchain_type adds a second toolchain() per cell, named <cell repo>_exec, of that type on the cell's exec platform.
    variants maps a variant to a struct of extra exec_compatible_with and target_compatible_with; each cell then has one toolchain(), named <cell repo>_<variant>, per variant, pointing at the cell's :<variant> target.
    companion_types maps a suffix to a toolchain type; every toolchain() gets a companion of that type, named <toolchain>_<suffix>, pointing at <target>_<suffix>.
    exec_aliases is a list of targets or a dict of alias to label in the cell repository; alias_defaults maps an alias to its label on platforms without a cell.
    repository names the one repository every cell points at in place of its own cell repository."""
    constraints = {
        cell: [_OS_CONSTRAINTS[cell.partition("_")[0]], _CPU_CONSTRAINTS[cell.partition("_")[2]]] + os_constraints.get(cell.partition("_")[0], [])
        for cell in cells
    }
    sides = ["exec", "target"] if compatible_with == "both" else [compatible_with]
    parts = []
    for cell in sorted(cells):
        repo = repository or cell_repo(name, cell)
        for variant in sorted(variants) or [""]:
            compatible = {
                side + "_compatible_with": constraints[cell] + (getattr(variants[variant], side + "_compatible_with") if variant else [])
                for side in sides
            }
            toolchain = cell_repo(name, cell) + ("_" + variant if variant else "")
            target = variant or "toolchain"
            parts.append(toolchain_target(
                name = toolchain,
                toolchain = "@{}//:{}".format(repo, target),
                toolchain_type = toolchain_type,
                visibility = ["//visibility:public"],
                **compatible
            ))
            parts.extend([
                toolchain_target(
                    name = toolchain + "_" + suffix,
                    toolchain = "@{}//:{}_{}".format(repo, target, suffix),
                    toolchain_type = companion_types[suffix],
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
                toolchain = "@{}//:toolchain".format(repository or cell_repo(name, cell)),
                toolchain_type = exec_toolchain_type,
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
        parts.extend([
            'alias(\n    name = "{}",\n    actual = select({}),\n    visibility = ["//visibility:public"],\n)\n'.format(
                alias,
                repr(
                    {":{}_exec".format(cell): "@{}{}".format(repository or cell_repo(name, cell), aliases[alias]) for cell in sorted(cells)} |
                    ({"//conditions:default": alias_defaults[alias]} if alias in alias_defaults else {}),
                ),
            )
            for alias in aliases
        ])
    return "\n".join(parts)
