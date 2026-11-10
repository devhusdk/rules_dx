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

_C_PACKAGE = {
    "extra": {
        "swift_products": {"Multi": ["Wrap", "Extra"]},
        "swift_targets": {
            "CCore": {
                "deps": [],
                "header_paths": ["Sources/CCore/private"],
                "include": "Sources/CCore/include",
                "kind": "clang",
                "module": "CCore",
                "path": "Sources/CCore",
                "product_deps": [],
                "srcs": ["Sources/CCore/ccore.c"],
            },
            "Wrap": {
                "defines": ["WRAP"],
                "deps": ["CCore"],
                "module": "Wrap",
                "path": "Sources/Wrap",
                "product_deps": [],
                "srcs": ["Sources/Wrap/Wrap.swift"],
            },
            "Extra": _target("Extra"),
        },
    },
    "name": "multi",
    "version": "1.0.0",
}

def _checks():
    c_text = build_file(_C_PACKAGE)
    text = build_file(_PACKAGE)
    return [
        expect_equal("a product named like its target has only the package alias", _SAME_NAME.count("alias("), 1),
        expect_equal("a product named like the package has no second alias", _PACKAGE_NAMED.count("alias("), 1),
        expect_equal("several products have no package label", 'name = "many"' in _MANY, False),
        expect_equal("no product leaves only the load", _NONE, 'load("%s", "swift_interop_hint", "swift_library", "swift_library_group")\nload("%s", "cc_library")\n' % (str(Label("//swift:defs.bzl")), str(Label("//cc:defs.bzl")))),
        expect_contains("a target is a swift_library", text, 'name = "LibCore",\n    module_name = "LibCore",\n    srcs = ["Sources/LibCore/a.swift"],'),
        expect_contains("a target and a product dependency are labels", text, 'deps = [":Helper", "//other:Other"],'),
        expect_contains("a product with another name is an alias", text, 'name = "Lib",\n    actual = ":LibCore",'),
        expect_contains("a C target is a cc_library with its include directory", c_text, 'name = "CCore",\n    srcs = ["Sources/CCore/ccore.c"] + glob(["Sources/CCore/private/**/*.h"]),\n    hdrs = glob(["Sources/CCore/include/**/*.h"]),\n    defines = [],\n    includes = ["Sources/CCore/include", "Sources/CCore/private"],'),
        expect_contains("a C target has an interop hint", c_text, 'aspect_hints = [":CCore_hint"],'),
        expect_contains("a Swift target has its defines", c_text, 'srcs = ["Sources/Wrap/Wrap.swift"],\n    defines = ["WRAP"],\n    deps = [":CCore"],'),
        expect_contains("a product with several targets is a group", c_text, 'swift_library_group(\n    name = "Multi",\n    deps = [":Wrap", ":Extra"],'),
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
