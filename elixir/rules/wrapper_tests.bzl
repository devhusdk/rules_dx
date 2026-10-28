"""Wrapper conformance tests for Elixir."""

load("//libs/starlark:conformance.bzl", "dx_wrapper_contract_tests")
load("//libs/starlark:defs.bzl", "expect_equal")
load(":defs.bzl", "DEFAULT_ELIXIRC_OPTS", "ELIXIR_CELLS", "ELIXIR_SOURCE_GLOBS", "app_kwargs", "app_mod_entry", "app_sources", "escript_headers", "helper_first")
load(":hex.bzl", "hex_metadata_entry", "hex_tarball_url")

def _glob(patterns, exclude = [], **_kwargs):
    return [",".join(patterns) + "!" + ",".join(exclude)]

def elixir_wrapper_contract_tests(name):
    """Instantiates Elixir wrapper contract tests."""
    dx_wrapper_contract_tests(
        name,
        file_classes = ["elixir"],
        extra_checks = [
            expect_equal("apps own lib sources", ELIXIR_SOURCE_GLOBS, ["lib/**/*.ex"]),
            expect_equal("default apps own the glob", app_sources({}, _glob), ["lib/**/*.ex!"]),
            expect_equal("explicit srcs are the owned files", app_sources({"srcs": ["lib/a.ex"]}, _glob), ["lib/a.ex"]),
            expect_equal("warnings fail the compile by default", app_kwargs({}, [])["elixirc_opts"], ["--warnings-as-errors"]),
            expect_equal("default options are the dx defaults", DEFAULT_ELIXIRC_OPTS, ["--warnings-as-errors"]),
            expect_equal("explicit options win", app_kwargs({"elixirc_opts": []}, [])["elixirc_opts"], []),
            expect_equal("owned sources reach the compiler", app_kwargs({"app_name": "a"}, ["lib/a.ex"]), {"app_name": "a", "elixirc_opts": ["--warnings-as-errors"], "srcs": ["lib/a.ex"]}),
            expect_equal("app modules are Elixir modules", app_mod_entry("Stock.Application"), "{mod, {'Elixir.Stock.Application', []}}"),
            expect_equal("app_module becomes the mod entry", app_kwargs({"app_module": "A"}, [])["app_extra_keys"], "{mod, {'Elixir.A', []}}"),
            expect_equal("app_module keeps other extra keys", app_kwargs({"app_extra_keys": "{x, 1}", "app_module": "A"}, [])["app_extra_keys"], "{mod, {'Elixir.A', []}},\n{x, 1}"),
            expect_equal("app_module is not passed upstream", "app_module" in app_kwargs({"app_module": "A"}, []), False),
            expect_equal("escripts start the Elixir module in a UTF-8 VM", escript_headers("Stockctl"), ["shebang", '{emu_args, "+fnu -kernel standard_io_encoding unicode -escript main Elixir.Stockctl"}']),
            expect_equal("test helpers are required first", helper_first(["test/a_test.exs", "test/test_helper.exs", "test/z_test.exs"]), ["test/test_helper.exs", "test/a_test.exs", "test/z_test.exs"]),
            expect_equal("a helper label is required first", helper_first(["//t:a_test.exs", "//t:test_helper.exs"]), ["//t:test_helper.exs", "//t:a_test.exs"]),
            expect_equal("srcs without a helper keep their order", helper_first(["b.exs", "a.exs"]), ["b.exs", "a.exs"]),
            expect_equal("apps compile on the Elixir cells", ELIXIR_CELLS, ["linux_amd64", "linux_arm64", "macos_arm64"]),
            expect_equal("hex tarballs come from repo.hex.pm", hex_tarball_url("jason", "1.4.4"), "https://repo.hex.pm/tarballs/jason-1.4.4.tar"),
            expect_equal("hex metadata names the release", hex_metadata_entry("version", "1.4.4"), '{<<"version">>,<<"1.4.4">>}.'),
        ],
    )
