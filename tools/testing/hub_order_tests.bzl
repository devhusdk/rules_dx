"""Toolchain count, registration order and toolchain types of a toolchain hub."""

load("//rust:defs.bzl", "rust_test")

def hub_order_test(name, build, names, toolchain_types, size = "small"):
    """Asserts the hub toolchain names in registration order and their toolchain types."""
    if names != sorted(names):
        fail("hub_order_test: names must be sorted, which is the registration order of //:all")
    if toolchain_types != sorted(toolchain_types):
        fail("hub_order_test: toolchain_types must be sorted")
    rust_test(
        name = name,
        size = size,
        srcs = [Label("//tools/testing:hub_order_test.rs")],
        crate_root = Label("//tools/testing:hub_order_test.rs"),
        data = [build],
        env = {
            "DX_HUB_BUILD": "$(rlocationpath {})".format(build),
            "DX_HUB_NAMES": " ".join(names),
            "DX_HUB_TOOLCHAIN_TYPES": " ".join(toolchain_types),
        },
        deps = [Label("//tools/testing:dx_testing")],
    )
