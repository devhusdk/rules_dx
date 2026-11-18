"""Unit tests of the elixirc replay command."""

load("@bazel_skylib//lib:unittest.bzl", "asserts", "unittest")
load("//core/quality:elixirc.bzl", "elixirc_command")

visibility("private")

def _command(env = {}):
    return elixirc_command(
        env_program = "bazel-out/coreutils_bin/env",
        elixir_home = "bazel-out/elixir",
        erlang_root = "bazel-out/erlang",
        coreutils_path = "bazel-out/coreutils_bin",
        erl_libs_path = "",
        env = env,
        out = "out",
        elixirc_opts = [],
        srcs = ["a.ex"],
    )

def _host_path_test_impl(ctx):
    env = unittest.begin(ctx)
    command = _command()
    asserts.equals(env, "PATH=__BAZEL_EXECUTION_ROOT__/bazel-out/elixir/bin:__BAZEL_EXECUTION_ROOT__/bazel-out/erlang/bin:__BAZEL_EXECUTION_ROOT__/bazel-out/coreutils_bin", command[1])
    return unittest.end(env)

def _env_test_impl(ctx):
    env = unittest.begin(ctx)
    command = _command({"A": "x y;z"})
    asserts.true(env, "A=x y;z" in command)
    asserts.equals(env, ["__BAZEL_EXECUTION_ROOT__/bazel-out/elixir/bin/elixirc", "-o", "out", "a.ex"], command[-4:])
    return unittest.end(env)

_host_path_test = unittest.make(_host_path_test_impl)
_env_test = unittest.make(_env_test_impl)

def elixirc_script_test_suite(name):
    unittest.suite(name, _host_path_test, _env_test)
