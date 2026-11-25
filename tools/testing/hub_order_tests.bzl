"""Toolchain count and registration order of a toolchain hub built by an upstream rule set."""

load("//rust:defs.bzl", "rust_test")

def hub_order_test(name, build, names, size = "small"):
    """Asserts that the hub BUILD file declares exactly the toolchains in names, which are in registration order."""
    if names != sorted(names):
        fail("hub_order_test: names must be sorted, which is the registration order of //:all")
    rust_test(
        name = name,
        size = size,
        srcs = [Label("//tools/testing:hub_order_test.rs")],
        crate_root = Label("//tools/testing:hub_order_test.rs"),
        data = [build],
        env = {
            "DX_HUB_BUILD": "$(rlocationpath {})".format(build),
            "DX_HUB_NAMES": " ".join(names),
        },
        deps = [Label("//tools/testing:dx_testing")],
    )
