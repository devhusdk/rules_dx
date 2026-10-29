"""Hermetic Sorbet acquisition: the pinned sorbet-static gem of the host, unpacked."""

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

def host_cell(os_name, arch):
    """Maps a repository_ctx host to a SORBET_CELLS key."""
    if os_name.startswith("mac"):
        os_key = "darwin"
    elif os_name.startswith("windows"):
        os_key = "windows"
    else:
        os_key = "linux"
    arch_key = "aarch64" if arch in ["aarch64", "arm64"] else "x86_64"
    return os_key + "_" + arch_key

def _sorbet_impl(rctx):
    cell = host_cell(rctx.os.name.lower(), rctx.os.arch)
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
    rctx.download_and_extract(
        url = url,
        output = "_gem",
        sha256 = pin.sha256,
        type = "tar",
        canonical_id = "dx-sorbet-gem:" + url,
    )
    if not rctx.path("_gem/data.tar.gz").exists:
        fail("sorbet: '" + url + "' holds no data.tar.gz")
    rctx.extract(archive = "_gem/data.tar.gz")
    rctx.delete("_gem")
    if not rctx.path(SORBET_BINARY).exists:
        fail("sorbet: '" + url + "' holds no " + SORBET_BINARY)
    rctx.file("BUILD.bazel", sorbet_build(SORBET_BINARY))

_sorbet = repository_rule(
    implementation = _sorbet_impl,
    doc = "Downloads and unpacks the pinned sorbet-static gem of the host platform.",
)

def _ruby_tools_impl(_mctx):
    _sorbet(name = "dx_sorbet")

ruby_tools = module_extension(
    implementation = _ruby_tools_impl,
    doc = "Creates the pinned Sorbet repository.",
)
