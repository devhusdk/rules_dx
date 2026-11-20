"""The checks on the rocks hub BUILD file."""

load("//core:pins.bzl", "lock_problems")
load("//core/private/testing:defs.bzl", "expect_contains", "expect_equal", "starlark_test")
load("//lua/private/lock:hub.bzl", "build_file", "luarocks_problems")

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

def _package(name, modules):
    return {"extra": {"lua_modules": modules}, "name": name, "version": "1-1"}

def _problems(*packages, **overrides):
    lock = {"ecosystem": "luarocks", "packages": list(packages), "schema": 1}
    lock.update(overrides)
    return lock_problems(lock) or luarocks_problems(lock)

def _lock_checks():
    return [
        expect_equal("a lock whose modules end in their paths has no problem", _problems(_package("pkg", {"pkg": "src/pkg.lua", "pkg.sub": "src/pkg/sub.lua"})), []),
        expect_equal("a package without lua_modules fails", _problems({"extra": {}, "name": "pkg", "version": "1-1"}), ["package pkg has no lua_modules object"]),
        expect_equal("a package with empty lua_modules fails", _problems(_package("pkg", {})), ["package pkg has no lua_modules object"]),
        expect_equal("a path that does not end in the module fails", _problems(_package("pkg", {"pkg.sub": "src/sub.lua"})), ['package pkg module pkg.sub has path "src/sub.lua", which does not end in pkg/sub.lua']),
        expect_equal("a path that only ends in the module name fails", _problems(_package("pkg", {"pkg": "src/mypkg.lua"})), ['package pkg module pkg has path "src/mypkg.lua", which does not end in pkg.lua']),
        expect_equal("another ecosystem fails", _problems(_package("pkg", {"pkg": "pkg.lua"}), ecosystem = "npm"), ['has ecosystem "npm", not luarocks']),
    ]

def _checks():
    return _lock_checks() + [
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
