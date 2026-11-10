"""The checks on the swiftpm hub BUILD file."""

load("//core:defs.bzl", "expect_contains", "expect_equal", "starlark_test")
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

def _target(name, deps = []):
    return {
        "deps": deps,
        "module": name,
        "path": "Sources/" + name,
        "product_deps": [],
        "srcs": ["Sources/%s/a.swift" % name],
    }

def _package(name, products, targets):
    return {
        "extra": {"swift_products": products, "swift_targets": targets},
        "name": name,
        "version": "1.0.0",
    }

_SAME_NAME = build_file(_package("same", {"Same": ["Same"]}, {"Same": _target("Same")}))
_MANY = build_file(_package("many", {"A": ["A"], "B": ["B"]}, {"A": _target("A"), "B": _target("B")}))
_NONE = build_file(_package("none", {}, {}))
_PACKAGE_NAMED = build_file(_package("A", {"A": ["Core"]}, {"Core": _target("Core")}))

def _checks():
    text = build_file(_PACKAGE)
    return [
        expect_equal("a product named like its target has only the package alias", _SAME_NAME.count("alias("), 1),
        expect_equal("a product named like the package has no second alias", _PACKAGE_NAMED.count("alias("), 1),
        expect_equal("several products have no package label", 'name = "many"' in _MANY, False),
        expect_equal("no product leaves only the load", _NONE, 'load("%s", "swift_library")\n' % str(Label("//swift:defs.bzl"))),
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
