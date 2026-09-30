"""Wrapper-contract tests for the JavaScript wrappers (item 2)."""

load("//libs/starlark:conformance.bzl", "dx_wrapper_contract_tests")
load("//libs/starlark:defs.bzl", "expect_equal")
load(":defs.bzl", "javascript_binary_upstream_data", "javascript_test_env", "javascript_test_rejection")

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
                "`auto_configure_reporters = False` is not supported.",
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
                "missing env gains the filter channel",
                javascript_test_env(None),
                ["TESTBRIDGE_TEST_ONLY"],
            ),
            expect_equal(
                "caller env keeps entries and gains the filter channel",
                javascript_test_env(["FOO"]),
                ["FOO", "TESTBRIDGE_TEST_ONLY"],
            ),
            expect_equal(
                "present filter channel is not duplicated",
                javascript_test_env(["FOO", "TESTBRIDGE_TEST_ONLY"]),
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
        ],
    )
