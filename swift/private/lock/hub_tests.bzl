"""The checks on the swiftpm hub BUILD file."""

load("//core:defs.bzl", "expect_contains", "starlark_test")
load(":hub.bzl", "build_file")

_PACKAGE = {
    "extra": {
        "swift_products": {"Lib": ["LibCore"]},
        "swift_targets": {
            "LibCore": {
                "deps": ["Helper"],
                "module": "LibCore",
                "path": "Sources/LibCore",
                "product_deps": ["other:Other"],
                "srcs": ["Sources/LibCore/a.swift"],
            },
            "Helper": {
                "deps": [],
                "module": "Helper",
                "path": "Sources/Helper",
                "product_deps": [],
                "srcs": ["Sources/Helper/b.swift"],
            },
        },
    },
    "name": "lib-pkg",
    "version": "1.0.0",
}

def _checks():
    text = build_file(_PACKAGE)
    return [
        expect_contains("a target is a swift_library", text, 'name = "LibCore",\n    module_name = "LibCore",\n    srcs = ["Sources/LibCore/a.swift"],'),
        expect_contains("a target and a product dependency are labels", text, 'deps = [":Helper", "//other:Other"],'),
        expect_contains("a product with another name is an alias", text, 'name = "Lib",\n    actual = ":LibCore",'),
        expect_contains("the only product is the package label", text, 'name = "lib-pkg",\n    actual = ":Lib",'),
    ]

def swift_hub_tests(name, size):
    """Declares the test that runs the swiftpm hub checks."""
    starlark_test(
        name = name,
        mode = "load",
        checks = _checks(),
        size = size,
    )
