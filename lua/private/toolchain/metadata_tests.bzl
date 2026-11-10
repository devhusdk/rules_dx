"""LuaLS pin tests."""

load("@dx_pins_data//:pins.bzl", "PINS")
load("//core:defs.bzl", "expect_equal", "expect_true", "starlark_test")
load("//core:host.bzl", "host_cell_of")

def _checks():
    pins = PINS["dx_luals"]
    return [
        expect_true("digests are sha256", all([len(c["sha256"]) == 64 for c in pins["cells"].values()])),
        expect_true("urls name the version", all([pins["version"] in c["urls"][0] for c in pins["cells"].values()])),
        expect_equal("pinned cells", sorted(pins["cells"].keys()), ["linux_arm64", "linux_x86_64", "macos_arm64", "macos_x86_64", "windows_x86_64"]),
        expect_equal("linux host", host_cell_of("linux", "amd64"), "linux_x86_64"),
        expect_equal("mac arm host", host_cell_of("mac os x", "aarch64"), "macos_arm64"),
        expect_equal("windows host", host_cell_of("windows server 2022", "amd64"), "windows_x86_64"),
    ]

def lua_metadata_tests(name):
    """Declares the LuaLS pin test."""
    starlark_test(
        name = name,
        mode = "load",
        checks = _checks(),
    )
