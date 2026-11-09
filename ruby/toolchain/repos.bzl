"""Hermetic Sorbet acquisition: the pinned sorbet-static gem of the host, unpacked."""

load("//libs/starlark:host.bzl", "host_cell")
load("//libs/starlark:repo.bzl", "gem_extract")
load(":pins.bzl", "SORBET_BINARY", "SORBET_CELLS", "sorbet_url")

_BUILD = """package(default_visibility = ["//visibility:public"])

exports_files(["{binary}"])
"""

_WINDOWS_STUB = """#!/bin/sh
echo "sorbet has no Windows build: the Ruby type check is not available on Windows" >&2
exit 2
"""

def sorbet_build(binary):
    """Returns the BUILD file of the Sorbet repository."""
    return _BUILD.format(binary = binary)

def _sorbet_impl(rctx):
    cell = host_cell(rctx)
    rctx.file("REPO.bazel", "")
    if cell.startswith("windows"):
        # No download: a stub keeps the label valid and fails loudly if it is ever run.
        rctx.file(SORBET_BINARY, _WINDOWS_STUB, executable = True)
        rctx.file("BUILD.bazel", sorbet_build(SORBET_BINARY))
        return
    pin = SORBET_CELLS.get(cell)
    if pin == None:
        fail("no pinned sorbet-static gem for " + cell)
    url = sorbet_url(pin.gem)
    gem_extract(
        rctx,
        url = url,
        sha256 = pin.sha256,
        canonical_id = "dx-sorbet-gem:" + url,
        output = ".",
        required = [SORBET_BINARY],
    )
    rctx.file("BUILD.bazel", sorbet_build(SORBET_BINARY))

sorbet_repo = repository_rule(
    implementation = _sorbet_impl,
    doc = "Downloads and unpacks the pinned sorbet-static gem of the host platform.",
)
