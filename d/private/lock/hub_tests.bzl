"""The checks on the dub hub BUILD file."""

load("//core:defs.bzl", "expect_contains", "expect_equal", "starlark_test")
load(":hub.bzl", "build_file")

_FULL = build_file({
    "deps": ["base"],
    "extra": {"d_library": {
        "imports": ["source"],
        "srcs": ["source/**/*.d", "source/**/*.di"],
        "string_imports": ["views"],
    }},
    "name": "silly",
    "version": "1.1.1",
})

_PLAIN = build_file({
    "extra": {"d_library": {"imports": ["lib"], "srcs": ["lib/**/*.d"]}},
    "name": "base",
    "version": "0.2.3",
})

def _checks():
    return [
        expect_contains("a package is a d_library of its name", _FULL, 'd_library(\n    name = "silly",'),
        expect_contains("the sources are globs", _FULL, 'srcs = glob(["source/**/*.d", "source/**/*.di"], allow_empty = True),'),
        expect_contains("a dependency is a label of the hub", _FULL, 'deps = ["//base"],'),
        expect_contains("the import paths are kept", _FULL, 'imports = ["source"],'),
        expect_contains("string imports bring their files", _FULL, 'string_srcs = glob(["views/**"]),'),
        expect_contains("a package without dependencies has an empty list", _PLAIN, "    deps = [],"),
        expect_contains("a package without string imports sets none", _PLAIN, "    imports = [\"lib\"],\n    visibility"),
        expect_contains("a package is public", _PLAIN, 'visibility = ["//visibility:public"],'),
        expect_equal("the load is the d defs", _PLAIN.split("\n")[0], 'load("%s", "d_library")' % str(Label("//d:defs.bzl"))),
    ]

def dub_hub_tests(name, size):
    """Declares the test that runs the dub hub checks."""
    starlark_test(
        name = name,
        mode = "load",
        checks = _checks(),
        size = size,
    )
