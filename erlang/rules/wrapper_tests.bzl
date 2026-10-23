"""Wrapper conformance tests for Erlang."""

load("//libs/starlark:conformance.bzl", "dx_wrapper_contract_tests")
load("//libs/starlark:defs.bzl", "expect_equal")
load(":defs.bzl", "ERLANG_COMPATIBLE_WITH", "ERLANG_SOURCE_GLOBS", "LOCAL_NODE_ARGS", "app_sources", "ct_run_args", "ct_suite_source", "runfiles_path")

def _glob(patterns, exclude = [], allow_empty = False):
    return [",".join(patterns) + "!" + ",".join(exclude)]

def erlang_wrapper_contract_tests(name):
    """Instantiates Erlang wrapper contract tests."""
    dx_wrapper_contract_tests(
        name,
        file_classes = ["erlang"],
        extra_checks = [
            expect_equal(
                "suites compile from test",
                ct_suite_source("kv_store_SUITE"),
                "test/kv_store_SUITE.erl",
            ),
            expect_equal(
                "apps own src and include sources",
                ERLANG_SOURCE_GLOBS,
                ["src/**/*.erl", "src/**/*.hrl", "include/**/*.hrl"],
            ),
            expect_equal("suites start no epmd", ct_run_args([]), ["-sname", "undefined", "-start_epmd", "false"]),
            expect_equal("suite args follow the local node", ct_run_args(["-ct_hooks cth_surefire"]), LOCAL_NODE_ARGS + ["-ct_hooks cth_surefire"]),
            expect_equal("named suites keep their node", ct_run_args(["-sname peer_host"]), ["-sname peer_host"]),
            expect_equal("long names keep their node", ct_run_args(["-name", "a@b"]), ["-name", "a@b"]),
            expect_equal("apps compile on the toolchain cell", [str(constraint) for constraint in ERLANG_COMPATIBLE_WITH], [str(Label("@platforms//os:linux")), str(Label("@platforms//cpu:x86_64"))]),
            expect_equal("default apps own the globs", app_sources({}, _glob), [",".join(ERLANG_SOURCE_GLOBS) + "!"]),
            expect_equal("explicit srcs are the owned files", app_sources({"srcs": ["lib/a.erl"], "hdrs": ["lib/a.hrl"]}, _glob), ["lib/a.erl", "lib/a.hrl"]),
            expect_equal("prebuilt beams own no sources", app_sources({"beam_files": [":beams"]}, _glob), []),
            expect_equal("extra sources leave the globs", app_sources({"extra_srcs": ["src/gen.erl", ":gen"]}, _glob), [",".join(ERLANG_SOURCE_GLOBS) + "!src/gen.erl"]),
            expect_equal(
                "main repo files live under the workspace",
                runfiles_path("_main", "examples/erlang-app/cli/kvctl.escript"),
                "_main/examples/erlang-app/cli/kvctl.escript",
            ),
            expect_equal(
                "external files live under their repo",
                runfiles_path("_main", "../+erlang+dx_otp_linux_amd64/DX_OTP_VERSION"),
                "+erlang+dx_otp_linux_amd64/DX_OTP_VERSION",
            ),
        ],
    )
