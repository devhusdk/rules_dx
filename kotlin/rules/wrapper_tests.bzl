"""Wrapper conformance tests for Kotlin."""

load("//libs/starlark:conformance.bzl", "dx_wrapper_contract_tests")
load("//libs/starlark:defs.bzl", "expect_equal")
load(":defs.bzl", "kotlin_kotlinc_opts_with_werror", "kotlin_test_class", "kotlin_test_entry_kwargs")

def kotlin_wrapper_contract_tests(name):
    """Instantiates Kotlin wrapper contract tests."""
    dx_wrapper_contract_tests(
        name,
        file_classes = ["kotlin", "java"],
        extra_checks = [
            expect_equal(
                "empty gains warnings-as-errors",
                kotlin_kotlinc_opts_with_werror("x", {})["kotlinc_opts"],
                str(Label("//kotlin/rules:warnings_as_errors")),
            ),
            expect_equal(
                "explicit opts are wrapped",
                kotlin_kotlinc_opts_with_werror("x", {"kotlinc_opts": "//custom:opts"})["kotlinc_opts"],
                ":x_kotlinc_opts",
            ),
            expect_equal(
                "none opts gain warnings-as-errors",
                kotlin_kotlinc_opts_with_werror("x", {"kotlinc_opts": None})["kotlinc_opts"],
                str(Label("//kotlin/rules:warnings_as_errors")),
            ),
            expect_equal(
                "other kwargs survive",
                kotlin_kotlinc_opts_with_werror("x", {"deps": [":hello_lib"]})["deps"],
                [":hello_lib"],
            ),
            expect_equal(
                "kotlin test root infers class",
                kotlin_test_class("FooTest", ["src/test/kotlin/hello/FooTest.kt"], "p"),
                "hello.FooTest",
            ),
            expect_equal(
                "java test root infers class",
                kotlin_test_class("FooTest", ["src/test/java/com/x/FooTest.kt"], "p"),
                "com.x.FooTest",
            ),
            expect_equal(
                "package root src infers class",
                kotlin_test_class("FooTest", ["FooTest.kt"], "kotlin/x"),
                "x.FooTest",
            ),
            expect_equal(
                "no name match infers nothing",
                kotlin_test_class("FooTest", ["src/test/kotlin/hello/Other.kt"], "p"),
                None,
            ),
            expect_equal(
                "non-list srcs infer nothing",
                kotlin_test_class("FooTest", None, "p"),
                None,
            ),
            expect_equal(
                "empty srcs infer nothing",
                kotlin_test_class("FooTest", [], "p"),
                None,
            ),
            expect_equal(
                "test gains inferred class",
                kotlin_test_entry_kwargs("FooTest", ["src/test/kotlin/hello/FooTest.kt"], "p", {}),
                {"test_class": "hello.FooTest"},
            ),
            expect_equal(
                "explicit test class is kept",
                kotlin_test_entry_kwargs("FooTest", ["src/test/kotlin/hello/FooTest.kt"], "p", {"test_class": "a.B"})["test_class"],
                "a.B",
            ),
            expect_equal(
                "empty test class counts as unset",
                kotlin_test_entry_kwargs("FooTest", ["src/test/kotlin/hello/FooTest.kt"], "p", {"test_class": ""})["test_class"],
                "hello.FooTest",
            ),
            expect_equal(
                "kotlinc opts survive inference",
                kotlin_test_entry_kwargs("FooTest", ["src/test/kotlin/hello/FooTest.kt"], "p", kotlin_kotlinc_opts_with_werror("FooTest", {}))["kotlinc_opts"],
                str(Label("//kotlin/rules:warnings_as_errors")),
            ),
        ],
    )
