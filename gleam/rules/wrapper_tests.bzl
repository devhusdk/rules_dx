"""Wrapper conformance tests for Gleam."""

load("//gleam/tools:gleam_tool.bzl", "gleam_tool_script")
load("//libs/starlark:conformance.bzl", "dx_wrapper_contract_tests")
load("//libs/starlark:defs.bzl", "expect_contains", "expect_equal", "expect_false", "expect_true")
load(":defs.bzl", "GLEAM_JS_SOURCE_GLOBS", "GLEAM_JS_TEST_GLOBS", "GLEAM_OTP_CELLS", "GLEAM_SOURCE_GLOBS", "GLEAM_TEST_GLOBS", "GLEAM_TEST_HELPER_GLOBS", "GLEEUNIT", "GLEEUNIT_JS", "owned_sources", "source_root", "test_helpers", "upstream_kwargs", "with_gleeunit")
load(":hex.bzl", "ffi_sources", "generated_erlang_module", "hex_metadata_entries", "hex_tarball_url", "js_label", "package_build")
load(":javascript.bzl", "gleam_toml", "module_path", "package_name", "source_path", "unique_trees")

def _glob(patterns, **_kwargs):
    return [",".join(patterns)]

def _file(short_path):
    return struct(short_path = short_path)

def _tree(path):
    return struct(path = path, basename = path.split("/")[-1])

_BUILD = package_build("lib", ["src/lib.gleam"], ["src/lib_ffi.erl"], ["src/lib_ffi.mjs"], ["@dep//:dep"])

def gleam_wrapper_contract_tests(name):
    """Instantiates Gleam wrapper contract tests."""
    dx_wrapper_contract_tests(
        name,
        file_classes = ["gleam"],
        extra_checks = [
            expect_equal("libraries own src sources", GLEAM_SOURCE_GLOBS, ["src/**/*.gleam"]),
            expect_equal("JavaScript libraries own their FFI", GLEAM_JS_SOURCE_GLOBS, ["src/**/*.gleam", "src/**/*.mjs"]),
            expect_equal("tests own test sources", GLEAM_TEST_GLOBS, ["test/**/*_test.gleam"]),
            expect_equal("test libraries glob every test module", GLEAM_TEST_HELPER_GLOBS, ["test/**/*.gleam"]),
            expect_equal("JavaScript tests own their helpers", GLEAM_JS_TEST_GLOBS, ["test/**/*.gleam"]),
            expect_equal("test helpers drop test modules", test_helpers(["test/a_test.gleam", "test/h.gleam", "test/sub/x.gleam"]), ["test/h.gleam", "test/sub/x.gleam"]),
            expect_equal("default sources come from the glob", owned_sources(None, GLEAM_SOURCE_GLOBS, _glob), ["src/**/*.gleam"]),
            expect_equal("explicit srcs are the owned files", owned_sources(["src/a.gleam"], GLEAM_SOURCE_GLOBS, _glob), ["src/a.gleam"]),
            expect_equal("module names start below the package src", source_root("app/core", "src"), "app/core/src"),
            expect_equal("a root package strips src", source_root("", "test"), "test"),
            expect_equal("warnings fail the compile by default", upstream_kwargs({}, [], "src", "p")["warnings_as_errors"], True),
            expect_equal("an explicit warnings choice wins", upstream_kwargs({"warnings_as_errors": False}, [], "src", "p")["warnings_as_errors"], False),
            expect_equal("an explicit prefix wins", upstream_kwargs({"strip_src_prefix": "x"}, [], "src", "p")["strip_src_prefix"], "x"),
            expect_equal("owned sources reach the compiler", upstream_kwargs({}, ["src/a.gleam"], "src", "p"), {"srcs": ["src/a.gleam"], "strip_src_prefix": "p/src", "warnings_as_errors": True}),
            expect_equal("tests get gleeunit once", [str(dep) for dep in with_gleeunit([":a"])], [":a", str(GLEEUNIT)]),
            expect_equal("a listed gleeunit is kept", [str(dep) for dep in with_gleeunit([str(GLEEUNIT)])], [str(GLEEUNIT)]),
            expect_equal("JavaScript tests get the JavaScript gleeunit", [str(dep) for dep in with_gleeunit(None, GLEEUNIT_JS)], [str(GLEEUNIT_JS)]),
            expect_equal("packages compile on the Gleam cells", GLEAM_OTP_CELLS, ["linux_amd64"]),
            expect_equal("package names come from the label", package_name("", "apps/stock-cli", "cli"), "apps_stock_cli_cli"),
            expect_equal("package names start with a letter", package_name("", "", "1st"), "p_1st"),
            expect_equal("package names fold case", package_name("My.Repo", "", "Lib"), "my_repo_lib"),
            expect_equal("gleam.toml lists direct deps", gleam_toml("app", ["b", "a"]), 'name = "app"\nversion = "0.0.0"\n\n[dependencies]\na = ">= 0.0.0"\nb = ">= 0.0.0"\n'),
            expect_equal("sources keep their module path", source_path(_file("app/src/a/b.gleam"), "app/src", "src"), "src/a/b.gleam"),
            expect_equal("external sources drop the repo", source_path(_file("../+gleam+dep/src/dict.mjs"), "src", "src"), "src/dict.mjs"),
            expect_equal("module paths drop the extension", module_path(_file("app/src/cli/main.gleam"), "app/src"), "cli/main"),
            expect_equal("trees sort by package", [tree.path for tree in unique_trees([_tree("b/x.gleam_js/zed"), _tree("a/y.gleam_js/alpha")])], ["a/y.gleam_js/alpha", "b/x.gleam_js/zed"]),
            expect_equal("a shared tree counts once", len(unique_trees([_tree("a/alpha"), _tree("a/alpha")])), 1),
            expect_equal("hex tarballs come from repo.hex.pm", hex_tarball_url("gleam_json", "3.1.0"), "https://repo.hex.pm/tarballs/gleam_json-3.1.0.tar"),
            expect_equal("hex metadata accepts both binary forms", hex_metadata_entries("name", "gleam_json"), ['{<<"name">>, <<"gleam_json">>}.', '{<<"name">>, <<"gleam_json"/utf8>>}.']),
            expect_equal("listed hex metadata", hex_metadata_entries("build_tools", "gleam", True)[0], '{<<"build_tools">>, [<<"gleam">>]}.'),
            expect_equal("nested modules map to @ names", generated_erlang_module("src/gleam/dict.gleam"), "gleam@dict"),
            expect_equal("generated Erlang is dropped", ffi_sources(["src/a.gleam", "src/a.erl", "src/a_ffi.erl", "src/gleam/b.gleam", "src/gleam@b.erl"]), ["src/a_ffi.erl"]),
            expect_contains("hex packages build for Erlang", _BUILD, 'name = "lib",'),
            expect_contains("hex packages keep their FFI", _BUILD, 'deps = [":lib_ffi", "@dep//:dep"],'),
            expect_contains("hex packages build for JavaScript", _BUILD, 'name = "lib_js",\n    package = "lib",\n    srcs = ["src/lib.gleam", "src/lib_ffi.mjs"],\n    deps = ["@dep//:dep_js"],'),
            expect_equal("explicit dep labels gain _js", js_label("@dep//pkg:lib"), "@dep//pkg:lib_js"),
            expect_equal("repo shorthand deps map to the repo target", js_label("@gleam_json"), "@gleam_json//:gleam_json_js"),
            expect_equal("package shorthand deps map to the package target", js_label("//pkg"), "//pkg:pkg_js"),
            expect_equal("repo package shorthand deps map to the package target", js_label("@dep//a/b"), "@dep//a/b:b_js"),
            expect_contains("hex packages compile with their own warnings", _BUILD, "warnings_as_errors = False,"),
            expect_equal("hex packages build only where Gleam runs", _BUILD.count("target_compatible_with = GLEAM_COMPATIBLE_WITH,"), 3),
            expect_contains("the tool runs its command", gleam_tool_script("gleam/bin/gleam", ["format"], []), "exec \"$runfiles/gleam/bin/gleam\" 'format' \"$@\""),
            expect_false("a tool without runtimes keeps PATH", "export PATH" in gleam_tool_script("g", [], [])),
            expect_true("runtimes go on PATH", 'export PATH="$(cd "$runfiles/otp/bin" && pwd):$(cd "$runfiles/node/bin" && pwd):$PATH"' in gleam_tool_script("g", [], ["otp/bin", "node/bin"])),
        ],
    )
