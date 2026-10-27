"""Wrapper-contract tests for the Python wrappers (item 2)."""

load("//libs/starlark:conformance.bzl", "dx_wrapper_contract_tests")
load("//libs/starlark:defs.bzl", "expect_equal")
load(":defs.bzl", "python_main", "python_main_rejection", "python_test_rejection")

def python_wrapper_contract_tests(name):
    """Instantiates Python wrapper contract tests."""
    dx_wrapper_contract_tests(
        name,
        include_forwarder_checks = False,
        extra_checks = [
            expect_equal(
                "generic main is rejected",
                python_test_rejection({"main": "main.py"}) != None,
                True,
            ),
            expect_equal(
                "main rejection names pytest",
                python_test_rejection({"main": "main.py"}),
                "python_test always runs pytest and provides its own " +
                "entrypoint; `main` is not supported. Use python_py_test " +
                "for a custom main.",
            ),
            expect_equal(
                "empty kwargs are clean",
                python_test_rejection({}),
                None,
            ),
            expect_equal(
                "ordinary test kwargs are clean",
                python_test_rejection({"deps": ["@pypi//pytest"], "tags": ["small"]}),
                None,
            ),
            expect_equal(
                "pytest-native entrypoint kwargs stay pytest",
                python_test_rejection({"pytest_args": ["-k foo"], "chdir": "pkg"}),
                None,
            ),
            expect_equal(
                "main matches the wrapper name",
                python_main("tool", ["tool.py", "tool_util.py"]),
                "tool.py",
            ),
            expect_equal(
                "main keeps the label form",
                python_main("tool", [":tool.py", "tool_util.py"]),
                ":tool.py",
            ),
            expect_equal(
                "main matches a nested src",
                python_main("tool", ["sub/tool.py", "tool_util.py"]),
                "sub/tool.py",
            ),
            expect_equal(
                "single src leaves main unset",
                python_main("tool", ["other.py"]),
                None,
            ),
            expect_equal(
                "empty srcs leave main unset",
                python_main("tool", []),
                None,
            ),
            expect_equal(
                "none srcs leave main unset",
                python_main("tool", None),
                None,
            ),
            expect_equal(
                "no main match is rejected",
                python_main_rejection("tool", ["a.py", "b.py"]),
                "has multiple srcs and none is tool.py; set main.",
            ),
            expect_equal(
                "several main matches are rejected",
                python_main_rejection("tool", ["tool.py", "sub/tool.py"]),
                "has multiple srcs matching tool.py: tool.py, sub/tool.py; set main.",
            ),
            expect_equal(
                "one main match is clean",
                python_main_rejection("tool", ["tool.py", "tool_util.py"]),
                None,
            ),
            expect_equal(
                "single src is clean",
                python_main_rejection("tool", ["other.py"]),
                None,
            ),
            expect_equal(
                "none srcs are clean",
                python_main_rejection("tool", None),
                None,
            ),
        ],
    )
