"""Wrapper conformance tests for Elixir."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("//libs/starlark:conformance.bzl", "dx_wrapper_contract_tests")
load("//libs/starlark:defs.bzl", "expect_equal")
load(":defs.bzl", "COVER_LOAD", "ELIXIR_CELLS", "ELIXIR_SOURCE_GLOBS", "app_kwargs", "app_mod_entry", "app_sources", "elixir_kwargs", "escript_headers", "helper_first", "nif_priv")
load(":hex.bzl", "hex_metadata_entry", "hex_tarball_url")
load(":mix_lock.bzl", "mix_lock_packages")

_MIX_LOCK = """%{
  # comment: with "quotes"
  "decimal": {:hex, :decimal, "2.1.1", "inner1", [:mix], [], "hexpm", "outer1"},
  "jason": {:hex, :jason, "1.4.4", "inner2", [:mix], [{:decimal, "~> 2.0", [hex: :decimal, repo: "hexpm", optional: false]}, {:opt, "~> 1.0", [hex: :opt_pkg, repo: "hexpm", optional: true]}], "hexpm", "outer2"},
}
"""

def _glob(patterns, exclude = [], **_kwargs):
    return [",".join(patterns) + "!" + ",".join(exclude)]

def elixir_wrapper_contract_tests(name):
    """Instantiates Elixir wrapper contract tests."""
    dx_wrapper_contract_tests(
        name,
        file_classes = ["elixir"],
        extra_checks = [
            expect_equal("elixir guard is a select by default", type(elixir_kwargs({})["target_compatible_with"]), "select"),
            expect_equal("elixir guard stays with a user list", type(elixir_kwargs({"target_compatible_with": ["//x:y"]})["target_compatible_with"]), "select"),
            expect_equal("elixir keeps user tags", elixir_kwargs({"tags": ["t"]})["tags"], ["t"]),
            expect_equal("apps own lib sources", ELIXIR_SOURCE_GLOBS, ["lib/**/*.ex"]),
            expect_equal("default apps own the glob", app_sources({}, _glob), ["lib/**/*.ex!"]),
            expect_equal("explicit srcs are the owned files", app_sources({"srcs": ["lib/a.ex"]}, _glob), ["lib/a.ex"]),
            expect_equal("warnings follow the repo flag by default", type(app_kwargs({}, [])["elixirc_opts"]), "select"),
            expect_equal("warnings can be forced on", app_kwargs({"warnings_as_errors": True}, [])["elixirc_opts"], ["--warnings-as-errors"]),
            expect_equal("warnings can be forced off", app_kwargs({"warnings_as_errors": False}, [])["elixirc_opts"], []),
            expect_equal("user options keep the flag", app_kwargs({"elixirc_opts": ["--no-docs"], "warnings_as_errors": True}, [])["elixirc_opts"], ["--no-docs", "--warnings-as-errors"]),
            expect_equal("owned sources reach the compiler", app_kwargs({"app_name": "a", "warnings_as_errors": False}, ["lib/a.ex"]), {"app_name": "a", "elixirc_opts": [], "srcs": ["lib/a.ex"]}),
            expect_equal("app modules are Elixir modules", app_mod_entry("Stock.Application"), "{mod, {'Elixir.Stock.Application', []}}"),
            expect_equal("app_module becomes the mod entry", app_kwargs({"app_module": "A"}, [])["app_extra_keys"], "{mod, {'Elixir.A', []}}"),
            expect_equal("app_module keeps other extra keys", app_kwargs({"app_extra_keys": "{x, 1}", "app_module": "A"}, [])["app_extra_keys"], "{mod, {'Elixir.A', []}},\n{x, 1}"),
            expect_equal("app_module is not passed upstream", "app_module" in app_kwargs({"app_module": "A"}, []), False),
            expect_equal("escripts start the Elixir module in a UTF-8 VM", escript_headers("Stockctl"), ["shebang", '{emu_args, "+fnu -kernel standard_io_encoding unicode -escript main Elixir.Stockctl"}']),
            expect_equal("test helpers are required first", helper_first(["test/a_test.exs", "test/test_helper.exs", "test/z_test.exs"]), ["test/test_helper.exs", "test/a_test.exs", "test/z_test.exs"]),
            expect_equal("a helper label is required first", helper_first(["//t:a_test.exs", "//t:test_helper.exs"]), ["//t:test_helper.exs", "//t:a_test.exs"]),
            expect_equal("the coverage load has no double quotes", COVER_LOAD.count('"'), 0),
            expect_equal("srcs without a helper keep their order", helper_first(["b.exs", "a.exs"]), ["b.exs", "a.exs"]),
            expect_equal("nifs land in priv after globbed and extra files", nif_priv({"extra_priv": ["x.txt"]}, ["//n:nif"], _glob), ["priv/**/*!x.txt", "x.txt", "//n:nif"]),
            expect_equal("label extra_priv is not excluded from the glob", nif_priv({"extra_priv": ["//p:f"]}, [], lambda patterns, exclude = [], **_k: exclude), ["//p:f"]),
            expect_equal("apps compile on the Elixir cells", ELIXIR_CELLS, ["linux_amd64", "linux_arm64", "macos_arm64", "windows_x86_64", "windows_arm64"]),
            expect_equal("hex tarballs come from repo.hex.pm", hex_tarball_url("jason", "1.4.4"), "https://repo.hex.pm/tarballs/jason-1.4.4.tar"),
            expect_equal("hex metadata names the release", hex_metadata_entry("version", "1.4.4"), '{<<"version">>,<<"1.4.4">>}.'),
            expect_equal("mix.lock hex entries become pins", [(p["package"], p["version"], p["sha256"]) for p in mix_lock_packages(_MIX_LOCK)], [("decimal", "2.1.1", "outer1"), ("jason", "1.4.4", "outer2")]),
            expect_equal("mix.lock keeps required deps only", [p["deps"] for p in mix_lock_packages(_MIX_LOCK)], [[], ["decimal"]]),
        ],
    )

def _coverage_env_impl(ctx):
    env = analysistest.begin(ctx)
    target = analysistest.target_under_test(env)
    run_env = target[RunEnvironmentInfo].environment if RunEnvironmentInfo in target else {}
    asserts.equals(env, ctx.attr.apps, run_env.get("DX_ELIXIR_COVER_APPS", ""))
    asserts.equals(env, ctx.attr.sources, run_env.get("DX_ELIXIR_COVER_SOURCES", ""))
    asserts.equals(env, ctx.attr.apps != "", "DX_ELIXIR_COVER_SCRIPT" in run_env)
    return analysistest.end(env)

_coverage_env_on_test = analysistest.make(
    _coverage_env_impl,
    attrs = {"apps": attr.string(), "sources": attr.string()},
    config_settings = {"//command_line_option:collect_code_coverage": True},
)

_coverage_env_off_test = analysistest.make(
    _coverage_env_impl,
    attrs = {"apps": attr.string(), "sources": attr.string()},
)

def elixir_coverage_env_test(name, target, apps, sources):
    """Asserts an elixir_test names its workspace apps and sources for the cover script under coverage, and only then."""
    _coverage_env_on_test(name = name, size = "small", target_under_test = target, apps = apps, sources = sources)
    _coverage_env_off_test(name = name + "_off", size = "small", target_under_test = target)
