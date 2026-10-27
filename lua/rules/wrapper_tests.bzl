"""Wrapper conformance tests for Lua."""

load("//libs/starlark:conformance.bzl", "dx_wrapper_contract_tests")
load("//libs/starlark:defs.bzl", "expect_equal")
load(
    ":defs.bzl",
    "LUA_ENTRY_SUFFIX",
    "LUA_SOURCE_SPECS",
    "canonical_rlocation",
    "entry_lines",
    "lua_deferred_tags",
    "lua_executable_name",
    "lua_quality_srcs",
    "module_name",
)

def lua_wrapper_contract_tests(name):
    """Instantiates Lua wrapper contract tests."""
    dx_wrapper_contract_tests(
        name,
        file_classes = ["lua"],
        extra_checks = [
            expect_equal("one lua class owns .lua files", LUA_SOURCE_SPECS, [("lua", ["lua"])]),
            expect_equal("a/b.lua is module a.b", module_name("a/b.lua"), "a.b"),
            expect_equal("a/init.lua is module a", module_name("a/init.lua"), "a"),
            expect_equal("a root init.lua is module init", module_name("init.lua"), "init"),
            expect_equal("a non-Lua file has no module", module_name("a/b.txt"), None),
            expect_equal("a dotted stem has no module", module_name("a/b.c.lua"), None),
            expect_equal(
                "a relative strip_import_prefix starts at the package",
                module_name("pkg/src/x/y.lua", "pkg", "src"),
                "x.y",
            ),
            expect_equal(
                "an absolute strip_import_prefix starts at the repository root",
                module_name("pkg/src/x/init.lua", "pkg", "/pkg/src/"),
                "x",
            ),
            expect_equal(
                "a file outside strip_import_prefix has no module",
                module_name("pkg/other/y.lua", "pkg", "src"),
                None,
            ),
            expect_equal(
                "import_prefix is added after stripping",
                module_name("pkg/src/y.lua", "pkg", "src", "vendor.lib"),
                "vendor.lib.y",
            ),
            expect_equal(
                "a root init.lua under import_prefix is the prefix",
                module_name("pkg/init.lua", "pkg", "/pkg", "inspect"),
                "inspect",
            ),
            expect_equal(
                "main repository files key under the workspace name",
                canonical_rlocation("lua/x.lua", "_main"),
                "_main/lua/x.lua",
            ),
            expect_equal(
                "external files key under their canonical repository",
                canonical_rlocation("../lua+/src/x.lua", "_main"),
                "lua+/src/x.lua",
            ),
            expect_equal(
                "the entry file lists main, the source repository and sorted modules",
                entry_lines("_main/a/main.lua", "", [("z", "_main/a/z.lua"), ("b.c", "_main/a/b/c.lua")]),
                "_main/a/main.lua\n\nb.c\t_main/a/b/c.lua\nz\t_main/a/z.lua\n",
            ),
            expect_equal("entry files end in .luaentry", LUA_ENTRY_SUFFIX, ".luaentry"),
            expect_equal("linux executables keep the target name", lua_executable_name("app", "launcher"), "app"),
            expect_equal("windows executables end in .exe", lua_executable_name("app", "launcher.exe"), "app.exe"),
            expect_equal("main leads the owned sources", lua_quality_srcs("main.lua", ["main.lua", "u.lua"]), ["main.lua", "u.lua"]),
            expect_equal(
                "deferred tags are added once",
                lua_deferred_tags(["no-lint", "manual"]),
                ["no-lint", "manual", "no-typecheck"],
            ),
        ],
    )
