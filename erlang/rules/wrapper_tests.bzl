"""Wrapper conformance tests for Erlang."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("//libs/starlark:conformance.bzl", "dx_wrapper_contract_tests")
load("//libs/starlark:defs.bzl", "expect_equal")
load(":defs.bzl", "ERLANG_CELLS", "ERLANG_SOURCE_GLOBS", "LOCAL_NODE_ARGS", "app_sources", "ct_run_args", "ct_suite_source", "lcov_apps_dirs", "runfiles_path")

def _glob(patterns, exclude = [], **_kwargs):
    return [",".join(patterns) + "!" + ",".join(exclude)]

def erlang_wrapper_contract_tests(name):
    """Instantiates Erlang wrapper contract tests."""
    dx_wrapper_contract_tests(
        name,
        file_classes = ["erlang"],
        extra_checks = [
            expect_equal("lcov finds example apps", lcov_apps_dirs(["examples/erlang-app/store/src/kv_store.erl", "examples/erlang-app/proto/src/p.erl"]), "examples/erlang-app"),
            expect_equal("lcov joins sorted roots", lcov_apps_dirs(["examples/erlang-app/store/src/kv_store.erl", "erlang/tests/runtime/src/dx_runtime_probe.erl"]), "erlang/tests:examples/erlang-app"),
            expect_equal("lcov skips non app sources", lcov_apps_dirs(["a/src/x.erl", "a/b/src/x.hrl", "lib/a.erl", "x/y/lib/z.erl"]), ""),
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
            expect_equal("apps compile on every OTP cell", ERLANG_CELLS, ["linux_amd64", "linux_arm64", "macos_arm64"]),
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

def _coverage_env_impl(ctx):
    env = analysistest.begin(ctx)
    target = analysistest.target_under_test(env)
    run_env = target[RunEnvironmentInfo].environment if RunEnvironmentInfo in target else {}
    got = run_env.get("COVERDATA_TO_LCOV_APPS_DIRS", "")
    asserts.equals(env, ctx.attr.apps_dirs, got)
    metadata = [f.short_path for f in target[InstrumentedFilesInfo].metadata_files.to_list()]
    if ctx.attr.apps_dirs:
        asserts.true(env, ctx.attr.source in metadata, "missing " + ctx.attr.source + " in " + str(metadata))
    return analysistest.end(env)

_coverage_env_on_test = analysistest.make(
    _coverage_env_impl,
    attrs = {"apps_dirs": attr.string(), "source": attr.string()},
    config_settings = {"//command_line_option:collect_code_coverage": True},
)

_coverage_env_off_test = analysistest.make(
    _coverage_env_impl,
    attrs = {"apps_dirs": attr.string(), "source": attr.string()},
)

def erlang_coverage_env_test(name, target, apps_dirs, source):
    """Asserts an Erlang test points coverdata_to_lcov at its app sources under coverage, and only then."""
    _coverage_env_on_test(name = name, size = "small", target_under_test = target, apps_dirs = apps_dirs, source = source)
    _coverage_env_off_test(name = name + "_off", size = "small", target_under_test = target, apps_dirs = "")
