"""Unit tests of the elixirc replay script."""

load("@bazel_skylib//lib:unittest.bzl", "asserts", "unittest")
load("//core/quality:elixirc.bzl", "elixirc_script")

def _script(env = {}):
    return elixirc_script(
        maybe_install_erlang = "",
        elixir_home = "bazel-out/elixir",
        erlang_home = "bazel-out/erlang",
        coreutils_path = "bazel-out/coreutils_bin",
        erl_libs_path = "",
        env = env,
        setup = "",
        elixirc_opts = [],
    )

def _host_path_test_impl(ctx):
    env = unittest.begin(ctx)
    script = _script()
    asserts.false(env, "/usr/bin" in script)
    asserts.false(env, ":/bin" in script)
    asserts.true(env, '"$ABS_COREUTILS"' in script)
    return unittest.end(env)

def _quoted_env_test_impl(ctx):
    env = unittest.begin(ctx)
    asserts.true(env, "export A='x y;z'" in _script({"A": "x y;z"}))
    return unittest.end(env)

_host_path_test = unittest.make(_host_path_test_impl)
_quoted_env_test = unittest.make(_quoted_env_test_impl)

def elixirc_script_test_suite(name):
    unittest.suite(name, _host_path_test, _quoted_env_test)
