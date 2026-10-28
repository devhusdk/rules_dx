"""Wrapper conformance tests for Erlang."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("//libs/starlark:conformance.bzl", "dx_wrapper_contract_tests")
load("//libs/starlark:defs.bzl", "expect_contains", "expect_equal")
load(":defs.bzl", "ERLANG_CELLS", "ERLANG_SOURCE_GLOBS", "LOCAL_NODE_ARGS", "app_sources", "ct_run_args", "ct_suite_source", "escript_nif_message", "lcov_apps_dirs", "native_priv_files", "nif_linkopt_cases", "nif_priv_path", "runfiles_path")
load(":providers.bzl", "BeamInfo")

def _glob(patterns, exclude = [], **_kwargs):
    return [",".join(patterns) + "!" + ",".join(exclude)]

def erlang_wrapper_contract_tests(name):
    """Instantiates Erlang wrapper contract tests."""
    dx_wrapper_contract_tests(
        name,
        file_classes = ["erlang"],
        extra_checks = [
            expect_equal("native priv keeps libraries", native_priv_files(["priv/a.so", "priv/b.dylib", "priv/c.dll", "priv/d.txt", "priv/e.so.1"]), ["priv/a.so", "priv/b.dylib", "priv/c.dll"]),
            expect_contains("nif refusal names label", escript_nif_message("//x:cli", [("app", "x/priv/a.so")]), "//x:cli"),
            expect_contains("nif refusal names app and file", escript_nif_message("//x:cli", [("app", "x/priv/a.so")]), "app: x/priv/a.so"),
            expect_contains("nif refusal says how to fix", escript_nif_message("//x:cli", []), "ship the app in a release or run it with erlang_app"),
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
            expect_equal("nifs land in the app priv directory", nif_priv_path("kv_checksum_nif"), "priv/kv_checksum_nif.so"),
            expect_equal("macos nifs resolve erts symbols at load", nif_linkopt_cases()[str(Label("@platforms//os:macos"))], ["-Wl,-undefined,dynamic_lookup"]),
            expect_equal("other nifs add no link options", nif_linkopt_cases()["//conditions:default"], []),
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
    native = ctx.attr.native and ctx.attr.apps_dirs != ""
    asserts.equals(env, native, "CC_CODE_COVERAGE_SCRIPT" in run_env)
    asserts.equals(env, False, "DX_COVERAGE_CLANG_OBJECTS" in run_env)
    metadata = [f.short_path for f in target[InstrumentedFilesInfo].metadata_files.to_list()]
    if ctx.attr.apps_dirs:
        asserts.true(env, ctx.attr.source in metadata, "missing " + ctx.attr.source + " in " + str(metadata))
    asserts.equals(env, native, len([m for m in metadata if m.endswith("runtime_objects_list.txt")]) > 0)
    return analysistest.end(env)

_coverage_env_on_test = analysistest.make(
    _coverage_env_impl,
    attrs = {"apps_dirs": attr.string(), "native": attr.bool(), "source": attr.string()},
    config_settings = {"//command_line_option:collect_code_coverage": True},
)

_coverage_env_off_test = analysistest.make(
    _coverage_env_impl,
    attrs = {"apps_dirs": attr.string(), "native": attr.bool(), "source": attr.string()},
)

def erlang_coverage_env_test(name, target, apps_dirs, source, native = False):
    """Asserts an Erlang test points coverdata_to_lcov at its app sources under coverage, and only then.

    With `native`, it also asserts the test collects LLVM coverage for the NIFs of its apps.
    """
    _coverage_env_on_test(name = name, size = "small", target_under_test = target, apps_dirs = apps_dirs, source = source, native = native)
    _coverage_env_off_test(name = name + "_off", size = "small", target_under_test = target, apps_dirs = "")

def _beam_info_impl(ctx):
    env = analysistest.begin(ctx)
    target = analysistest.target_under_test(env)
    asserts.true(env, BeamInfo in target, "erlang_app must carry BeamInfo")
    info = target[BeamInfo]
    owned = sorted([f.basename for f in info.owned.to_list()])
    asserts.equals(env, ctx.attr.owned, owned)
    asserts.equals(env, ctx.attr.app_name, info.app_name)
    return analysistest.end(env)

_beam_info_test = analysistest.make(
    _beam_info_impl,
    attrs = {"app_name": attr.string(), "owned": attr.string_list()},
)

def erlang_beam_info_test(name, target, app_name, owned):
    """Asserts an erlang_app exports its compiled beams in BeamInfo."""
    _beam_info_test(name = name, size = "small", target_under_test = target, app_name = app_name, owned = owned)

def _nif_refusal_impl(ctx):
    env = analysistest.begin(ctx)
    asserts.expect_failure(env, ctx.attr.message)
    return analysistest.end(env)

_nif_refusal_test = analysistest.make(
    _nif_refusal_impl,
    expect_failure = True,
    attrs = {"message": attr.string(mandatory = True)},
)

def erlang_escript_nif_refusal_test(name, target, message, **kwargs):
    """Asserts an escript_archive that bundles a NIF app fails analysis with `message`."""
    _nif_refusal_test(name = name, size = "small", target_under_test = target, message = message, **kwargs)
