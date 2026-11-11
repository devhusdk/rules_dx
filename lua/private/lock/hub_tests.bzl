"""The checks on the rocks hub BUILD file."""

load("//core:defs.bzl", "expect_contains", "expect_equal", "starlark_test")
load(":hub.bzl", "build_file")

_NESTED = build_file({
    "deps": ["inspect", "known"],
    "extra": {"lua_modules": {"pkg": "src/pkg.lua", "pkg.sub": "src/pkg/sub.lua"}},
    "name": "pkg",
    "version": "1.0-1",
})

_FLAT = build_file({
    "extra": {"lua_modules": {"known": "known.lua"}},
    "name": "known",
    "version": "1-1",
})

def _checks():
    return [
        expect_contains("a package is a lua_library of its name", _NESTED, 'lua_library(\n    name = "pkg",'),
        expect_contains("the sources are the module files", _NESTED, 'srcs = ["src/pkg.lua", "src/pkg/sub.lua"],'),
        expect_contains("module names start below the source directory", _NESTED, 'strip_import_prefix = "/pkg/src",'),
        expect_contains("a dependency is a label of the hub", _NESTED, 'deps = ["//inspect", "//known"],'),
        expect_contains("a flat package strips only its own directory", _FLAT, 'strip_import_prefix = "/known",'),
        expect_contains("a package without dependencies has an empty list", _FLAT, "    deps = [],"),
        expect_contains("a package is public", _FLAT, 'visibility = ["//visibility:public"],'),
        expect_equal("the load is the lua defs", _FLAT.split("\n")[0], 'load("%s", "lua_library")' % str(Label("//lua:defs.bzl"))),
    ]

def lua_hub_tests(name, size):
    """Declares the test that runs the rocks hub checks."""
    starlark_test(
        name = name,
        mode = "load",
        checks = _checks(),
        size = size,
    )
