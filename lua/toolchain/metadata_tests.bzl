"""LuaLS pin tests."""

load("//libs/starlark:defs.bzl", "expect_contains", "expect_equal", "expect_true", "starlark_test")
load(":pins.bzl", "LUALS_CELLS", "LUALS_VERSION")
load(":repos.bzl", "host_cell", "luals_build")

def _checks():
    return [
        expect_equal("luals version", LUALS_VERSION, "3.19.2"),
        expect_equal("pinned cells", sorted(LUALS_CELLS.keys()), ["darwin_aarch64", "darwin_x86_64", "linux_aarch64", "linux_x86_64", "windows_x86_64"]),
        expect_true("digests are sha256", all([len(c.sha256) == 64 for c in LUALS_CELLS.values()])),
        expect_true("urls name the version and archive", all([LUALS_VERSION in c.urls[0] and c.urls[0].endswith(c.archive) for c in LUALS_CELLS.values()])),
        expect_equal("linux host", host_cell("linux", "amd64"), "linux_x86_64"),
        expect_equal("mac arm host", host_cell("mac os x", "aarch64"), "darwin_aarch64"),
        expect_equal("windows host", host_cell("windows server 2022", "amd64"), "windows_x86_64"),
        expect_contains("the repo exports the binary", luals_build("bin/lua-language-server"), 'exports_files(["bin/lua-language-server"])'),
    ]

def lua_metadata_tests(name):
    """Declares the LuaLS pin test."""
    starlark_test(
        name = name,
        mode = "load",
        checks = _checks(),
    )
