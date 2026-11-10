"""LuaLS pin tests."""

load("//core:defs.bzl", "expect_contains", "expect_equal", "starlark_test")
load("//core:host.bzl", "host_cell_of")
load("//core:toolchain.bzl", "pin_checks")
load(":pins.bzl", "LUALS_CELLS", "LUALS_VERSION")
load(":repos.bzl", "LUALS_BUILD")

def _checks():
    return pin_checks(LUALS_CELLS, LUALS_VERSION) + [
        expect_equal("linux host", host_cell_of("linux", "amd64"), "linux_x86_64"),
        expect_equal("mac arm host", host_cell_of("mac os x", "aarch64"), "macos_arm64"),
        expect_equal("windows host", host_cell_of("windows server 2022", "amd64"), "windows_x86_64"),
        expect_contains("the repo exports the binary", LUALS_BUILD, 'exports_files(["{binary}"])'),
    ]

def lua_metadata_tests(name):
    """Declares the LuaLS pin test."""
    starlark_test(
        name = name,
        mode = "load",
        checks = _checks(),
    )
