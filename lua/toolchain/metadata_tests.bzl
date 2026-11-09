"""LuaLS pin tests."""

load("//libs/starlark:defs.bzl", "expect_contains", "expect_equal", "expect_true", "starlark_test")
load("//libs/starlark:host.bzl", "host_cell_of")
load(":pins.bzl", "LUALS_CELLS", "LUALS_VERSION")
load(":repos.bzl", "luals_build")

def _checks():
    return [
        expect_true("digests are sha256", all([len(c.sha256) == 64 for c in LUALS_CELLS.values()])),
        expect_true("urls name the version and archive", all([LUALS_VERSION in c.urls[0] and c.urls[0].endswith(c.archive) for c in LUALS_CELLS.values()])),
        expect_equal("linux host", host_cell_of("linux", "amd64"), "linux_x86_64"),
        expect_equal("mac arm host", host_cell_of("mac os x", "aarch64"), "macos_arm64"),
        expect_equal("windows host", host_cell_of("windows server 2022", "amd64"), "windows_x86_64"),
        expect_contains("the repo exports the binary", luals_build("bin/lua-language-server"), 'exports_files(["bin/lua-language-server"])'),
    ]

def lua_metadata_tests(name):
    """Declares the LuaLS pin test."""
    starlark_test(
        name = name,
        mode = "load",
        checks = _checks(),
    )
