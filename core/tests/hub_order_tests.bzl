"""Toolchain count and registration order of a toolchain hub built by an upstream rule set."""

load("@bazel_skylib//rules:diff_test.bzl", "diff_test")

def hub_order_test(name, hub, names, size = "small"):
    """Asserts that hub//:all holds exactly the toolchains in names, which are in registration order."""
    if names != sorted(names):
        fail("hub_order_test: names must be sorted, which is the registration order of //:all")
    labels = ["{}//:{}".format(hub, n) for n in names]
    native.genquery(
        name = name + "_query",
        expression = "kind(toolchain, {}//:all)".format(hub),
        opts = ["--output=label"],
        scope = labels,
    )
    native.genquery(
        name = name + "_expected_query",
        expression = " + ".join(labels),
        opts = ["--output=label"],
        scope = labels,
    )
    diff_test(
        name = name,
        size = size,
        file1 = ":" + name + "_expected_query",
        file2 = ":" + name + "_query",
    )
