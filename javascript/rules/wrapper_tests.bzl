"""Wrapper-contract tests for the JavaScript wrappers (item 2)."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("//libs/starlark:conformance.bzl", "dx_wrapper_contract_tests")
load("//libs/starlark:defs.bzl", "expect_equal")
load("//libs/starlark:wrapper.bzl", "dx_args_label_error", "dx_args_label_references")
load(":defs.bzl", "javascript_binary_upstream_data", "javascript_js_test_rejection", "javascript_test_rejection")
load(":node_test.bzl", "node_test_env", "node_test_upstream_data")

def javascript_wrapper_contract_tests(name):
    """Instantiates JavaScript wrapper contract tests."""
    dx_wrapper_contract_tests(
        name,
        include_forwarder_checks = False,
        extra_checks = [
            expect_equal(
                "disabling standard reporters is rejected",
                javascript_test_rejection({"auto_configure_reporters": False}) != None,
                True,
            ),
            expect_equal(
                "reporter rejection names the standard logs",
                javascript_test_rejection({"auto_configure_reporters": False}),
                "javascript_test always uses jest with the standard " +
                "auto-configured reporters (Bazel test logs); " +
                "`auto_configure_reporters = False` is not supported. " +
                "Use javascript_js_test for a custom runner.",
            ),
            expect_equal(
                "empty kwargs are clean",
                javascript_test_rejection({}),
                None,
            ),
            expect_equal(
                "explicit standard reporters are clean",
                javascript_test_rejection({"auto_configure_reporters": True}),
                None,
            ),
            expect_equal(
                "ordinary jest kwargs are clean",
                javascript_test_rejection({"config": "jest.config.cjs", "snapshots": False}),
                None,
            ),
            expect_equal(
                "plain runner rejects the managed node_modules label",
                javascript_js_test_rejection({"node_modules": "//:node_modules"}) != None,
                True,
            ),
            expect_equal(
                "plain runner rejection names the jest route",
                javascript_js_test_rejection({"node_modules": "//:node_modules"}),
                "javascript_js_test runs plain Node via js_test without " +
                "Jest reporting; `node_modules` is not supported. " +
                "Use javascript_test for Jest behavior.",
            ),
            expect_equal(
                "plain runner rejects the jest config",
                javascript_js_test_rejection({"config": "jest.config.cjs"}) != None,
                True,
            ),
            expect_equal(
                "plain runner rejects disabled jest reporters",
                javascript_js_test_rejection({"auto_configure_reporters": True}) != None,
                True,
            ),
            expect_equal(
                "plain runner keeps ordinary node kwargs",
                javascript_js_test_rejection({"node_options": ["--experimental-vm-modules"], "env": {"DX_PLAIN": "1"}}),
                None,
            ),
            expect_equal(
                "plain runner with empty kwargs is clean",
                javascript_js_test_rejection({}),
                None,
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
                node_test_upstream_data(["a.js", "//javascript/rules:package_json"], ":package_json", native.package_relative_label),
                ["a.js", "//javascript/rules:package_json"],
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
                dx_args_label_error("t", ["$(location //javascript/rules:d)"], [":a.js"], {"deps": [], "tools": [":d"]}),
                "t: args names //javascript/rules:d, which is only in tools; add it to data",
            ),
            expect_equal(
                "label also in data is accepted",
                dx_args_label_error("t", ["$(rootpath :d)"], [":a.js", "//javascript/rules:d"], {"deps": [":d"]}),
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
