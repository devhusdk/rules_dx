"""The checks on the zig hub BUILD file."""

load("//core:defs.bzl", "expect_contains", "expect_equal", "starlark_test")
load(":hub.bzl", "build_file")

_WITH_DEPS = build_file({
    "deps": ["xxhash", "known"],
    "extra": {"zig_main": "src/root.zig"},
    "name": "clap",
    "version": "clap-05faf39",
})

_NO_DEPS = build_file({
    "extra": {"zig_main": "main.zig"},
    "name": "known",
    "version": "known-1",
})

def _checks():
    return [
        expect_contains("a package is a zig_library of its name", _WITH_DEPS, 'zig_library(\n    name = "clap",'),
        expect_contains("the main file is excluded from the sources", _WITH_DEPS, 'exclude = ["build.zig", "src/root.zig"]'),
        expect_contains("the package name is the import name", _WITH_DEPS, '    import_name = "clap",\n    main = "src/root.zig",'),
        expect_contains("a dependency is a label of the hub", _WITH_DEPS, 'deps = ["//xxhash", "//known"],'),
        expect_contains("a package without dependencies has an empty list", _NO_DEPS, "    deps = [],"),
        expect_contains("a package is public", _NO_DEPS, 'visibility = ["//visibility:public"],'),
        expect_equal("the load is the zig defs", _NO_DEPS.split("\n")[0], 'load("%s", "zig_library")' % str(Label("//zig:defs.bzl"))),
    ]

def zig_hub_tests(name, size):
    """Declares the test that runs the zig hub checks."""
    starlark_test(
        name = name,
        mode = "load",
        checks = _checks(),
        size = size,
    )
