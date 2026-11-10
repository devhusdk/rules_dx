"""Wrapper-contract tests for the JavaScript wrappers (item 2)."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("//libs/starlark:conformance.bzl", "dx_wrapper_contract_tests")
load("//libs/starlark:defs.bzl", "expect_equal")
load("//libs/starlark:wrapper.bzl", "dx_args_label_error", "dx_args_label_references")
load(":defs.bzl", "javascript_binary_upstream_data", "javascript_test_rejection")
load("//libs/starlark:node_test.bzl", "NODE_JEST_WINDOWS_ARGS", "node_jest_upstream_attrs", "node_test_env", "node_test_upstream_data", "node_test_wrap_args")

def _wrap_args(kwargs):
    return node_test_wrap_args("t", ["a.js"], ["a.js"], None, {}, None, kwargs)

def _jest_kwargs_after_attrs():
    kwargs = {"fixed_args": ["--ci"], "size": "small"}
    node_jest_upstream_attrs("//:nm", kwargs)
    return kwargs

def javascript_wrapper_contract_tests(name):
    """Instantiates JavaScript wrapper contract tests."""
    dx_wrapper_contract_tests(
        name,
        include_forwarder_checks = False,
        extra_checks = [
            expect_equal(
                "unknown runner is rejected",
                javascript_test_rejection("vitest", "//:nm", None, {}),
                "javascript_test runner must be \"jest\" or \"node\", got \"vitest\".",
            ),
            expect_equal(
                "jest needs node_modules",
                javascript_test_rejection("jest", None, None, {}),
                "javascript_test with runner = \"jest\" needs node_modules.",
            ),
            expect_equal(
                "jest rejects main",
                javascript_test_rejection("jest", "//:nm", "main.js", {}),
                "javascript_test with runner = \"jest\" runs jest; set runner = \"node\" to use main.",
            ),
            expect_equal(
                "jest rejects disabled reporters",
                javascript_test_rejection("jest", "//:nm", None, {"auto_configure_reporters": False}),
                "javascript_test with runner = \"jest\" keeps the standard reporters; set runner = \"node\" for a custom runner.",
            ),
            expect_equal(
                "node needs main",
                javascript_test_rejection("node", None, None, {}),
                "javascript_test with runner = \"node\" needs main.",
            ),
            expect_equal(
                "node rejects node_modules",
                javascript_test_rejection("node", "//:nm", "main.js", {}),
                "javascript_test with runner = \"node\" runs plain Node; `node_modules` needs runner = \"jest\".",
            ),
            expect_equal(
                "node rejects the jest config",
                javascript_test_rejection("node", None, "main.js", {"config": "jest.config.cjs"}),
                "javascript_test with runner = \"node\" runs plain Node; `config` needs runner = \"jest\".",
            ),
            expect_equal(
                "jest with ordinary kwargs is clean",
                javascript_test_rejection("jest", "//:nm", None, {"config": "jest.config.cjs", "snapshots": False, "auto_configure_reporters": True}),
                None,
            ),
            expect_equal(
                "node with ordinary kwargs is clean",
                javascript_test_rejection("node", None, "main.js", {"node_options": ["--experimental-vm-modules"], "env": {"DX_PLAIN": "1"}}),
                None,
            ),
            expect_equal(
                "wrap args drop testonly",
                "testonly" in _wrap_args({"testonly": True, "size": "medium"}).upstream_kwargs,
                False,
            ),
            expect_equal(
                "wrap args keep size",
                _wrap_args({"size": "medium"}).upstream_kwargs["size"],
                "medium",
            ),
            expect_equal(
                "wrap args keep aspect_hints",
                _wrap_args({"aspect_hints": ["x"]}).upstream_kwargs["aspect_hints"],
                ["x"],
            ),
            expect_equal(
                "wrap args inherit TESTBRIDGE_TEST_ONLY",
                _wrap_args({}).kwargs["env_inherit"],
                ["TESTBRIDGE_TEST_ONLY"],
            ),
            expect_equal(
                "forwarder data is unset without args or env",
                _wrap_args({}).extra_forward_kwargs,
                {},
            ),
            expect_equal(
                "forwarder data is set with args",
                _wrap_args({"args": ["--x"]}).extra_forward_kwargs,
                {"data": ["a.js"]},
            ),
            expect_equal(
                "forwarder data is set with env",
                _wrap_args({"env": {"A": "1"}}).extra_forward_kwargs,
                {"data": ["a.js"]},
            ),
            expect_equal(
                "the Windows jest default selects the v8 coverage provider",
                NODE_JEST_WINDOWS_ARGS,
                ["--coverageProvider=v8"],
            ),
            expect_equal(
                "jest attrs take fixed_args out of kwargs",
                _jest_kwargs_after_attrs(),
                {"size": "small"},
            ),
            expect_equal(
                "unset package_json adds no manifest",
                node_test_upstream_data(["a.js"], None, native.package_relative_label),
                ["a.js"],
            ),
            expect_equal(
                "set package_json is appended",
                node_test_upstream_data(["a.js"], "//:package_json", native.package_relative_label),
                ["a.js", "//:package_json"],
            ),
            expect_equal(
                "package_json already in data is not added twice",
                node_test_upstream_data(["a.js", "//:package_json"], "//:package_json", native.package_relative_label),
                ["a.js", "//:package_json"],
            ),
            expect_equal(
                "relative and absolute spellings of one manifest are not added twice",
                node_test_upstream_data(["a.js", "//javascript/private:package_json"], ":package_json", native.package_relative_label),
                ["a.js", "//javascript/private:package_json"],
            ),
            expect_equal(
                "missing env gains the filter channel",
                node_test_env(None),
                ["TESTBRIDGE_TEST_ONLY"],
            ),
            expect_equal(
                "caller env keeps entries and gains the filter channel",
                node_test_env(["FOO"]),
                ["FOO", "TESTBRIDGE_TEST_ONLY"],
            ),
            expect_equal(
                "present filter channel is not duplicated",
                node_test_env(["FOO", "TESTBRIDGE_TEST_ONLY"]),
                ["FOO", "TESTBRIDGE_TEST_ONLY"],
            ),
            expect_equal(
                "thin binary keeps upstream data unchanged",
                javascript_binary_upstream_data([], [":main"]),
                [":main"],
            ),
            expect_equal(
                "thin binary with no data stays empty",
                javascript_binary_upstream_data([], None),
                [],
            ),
            expect_equal(
                "ordinary binary srcs ride upstream as data",
                javascript_binary_upstream_data(["app.js"], [":hello_lib"]),
                ["app.js", ":hello_lib"],
            ),
            expect_equal(
                "ordinary binary srcs without data become data",
                javascript_binary_upstream_data(["app.js"], None),
                ["app.js"],
            ),
            expect_equal(
                "args scan finds location-style labels",
                dx_args_label_references(["--a=$(rootpath :x)", "$(locations //p:y) $(execpath @r//:z)", "$(PATH)", "plain"]),
                [":x", "//p:y", "@r//:z"],
            ),
            expect_equal(
                "label only in deps is an error naming deps",
                dx_args_label_error("t", ["$(rootpath :d)"], [":a.js"], {"deps": [":d"], "tools": None}),
                "t: args names :d, which is only in deps; add it to data",
            ),
            expect_equal(
                "label only in tools is an error naming tools",
                dx_args_label_error("t", ["$(location //javascript/private:d)"], [":a.js"], {"deps": [], "tools": [":d"]}),
                "t: args names //javascript/private:d, which is only in tools; add it to data",
            ),
            expect_equal(
                "label also in data is accepted",
                dx_args_label_error("t", ["$(rootpath :d)"], [":a.js", "//javascript/private:d"], {"deps": [":d"]}),
                None,
            ),
            expect_equal(
                "no args is accepted",
                dx_args_label_error("t", None, [], {"deps": [":d"]}),
                None,
            ),
        ],
    )

def _run_environment_forward_impl(ctx):
    env = analysistest.begin(ctx)
    run_env = analysistest.target_under_test(env)[RunEnvironmentInfo]
    asserts.equals(env, "1", run_env.environment.get("FOO"))
    asserts.true(env, "TESTBRIDGE_TEST_ONLY" in run_env.inherited_environment, str(run_env.inherited_environment))
    return analysistest.end(env)

_run_environment_forward_test = analysistest.make(_run_environment_forward_impl)

def javascript_run_environment_tests(name, target_under_test):
    """Asserts the Node forwarder merges the upstream RunEnvironmentInfo."""
    _run_environment_forward_test(name = name, target_under_test = target_under_test)
