"""Hermetic Sorbet acquisition: the pinned sorbet-static gem of the host, unpacked."""

load("//libs/starlark:host.bzl", "host_cell")
load("//libs/starlark:repo.bzl", "gem_extract", "pinned_archive")
load(":pins.bzl", "RULES_RUBY_SHA256", "RULES_RUBY_URL", "RULES_RUBY_VERSION", "SORBET_BINARY", "SORBET_CELLS", "sorbet_url")

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

_sorbet = repository_rule(
    implementation = _sorbet_impl,
    doc = "Downloads and unpacks the pinned sorbet-static gem of the host platform.",
)

def _ruby_tools_impl(mctx):
    _sorbet(name = "dx_sorbet")
    pinned_archive(
        name = "rules_ruby",
        crlf = ["ruby/private/binary/binary.cmd.tpl"],
        dropped = ["MODULE.bazel"],
        patches = [
            Label("//patches/rules_ruby:reproducible.patch"),
            Label("//patches/rules_ruby:coverage.patch"),
        ],
        sha256 = RULES_RUBY_SHA256,
        strip_prefix = "rules_ruby-" + RULES_RUBY_VERSION,
        urls = [RULES_RUBY_URL],
    )
    return mctx.extension_metadata(reproducible = True)

ruby_tools = module_extension(
    implementation = _ruby_tools_impl,
    doc = "Creates the pinned Sorbet repository and the patched rules_ruby.",
)
