"""LuaLS pin and Lua toolchain hub tests."""

load("@dx_pins_data//:pins.bzl", "PINS")
load("//core:defs.bzl", "expect_equal", "expect_true", "starlark_test")
load("//core:host.bzl", "host_cell_of")
load("//core:toolchain.bzl", "hub_checks")
load("//lua/private/toolchain:extensions.bzl", "lua_hub")

def _checks():
    pins = PINS["dx_luals"]
    hub = lua_hub()
    return hub_checks(hub, "//lua/private/toolchain:toolchain_type", 1) + [
        expect_true("hub names the lua toolchain", "//lua/private/toolchain:lua\"" in hub),
        expect_true("digests are sha256", all([len(c["sha256"]) == 64 for c in pins["cells"].values()])),
        expect_true("urls name the version", all([pins["version"] in c["urls"][0] for c in pins["cells"].values()])),
        expect_equal("pinned cells", sorted(pins["cells"].keys()), ["linux_arm64", "linux_x86_64", "macos_arm64", "macos_x86_64", "windows_x86_64"]),
        expect_equal("linux host", host_cell_of("linux", "amd64"), "linux_x86_64"),
        expect_equal("mac arm host", host_cell_of("mac os x", "aarch64"), "macos_arm64"),
        expect_equal("windows host", host_cell_of("windows server 2022", "amd64"), "windows_x86_64"),
    ]

def lua_metadata_tests(name, size):
    """Declares the LuaLS pin and Lua toolchain hub test."""
    starlark_test(
        name = name,
        mode = "load",
        checks = _checks(),
        size = size,
    )
