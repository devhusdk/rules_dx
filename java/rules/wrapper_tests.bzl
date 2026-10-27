"""Wrapper conformance tests for Java."""

load("//libs/starlark:conformance.bzl", "dx_wrapper_contract_tests")
load("//libs/starlark:defs.bzl", "expect_equal")
load(":defs.bzl", "java_entry_class", "java_entry_kwargs", "java_javacopts_with_werror")

def java_wrapper_contract_tests(name):
    """Instantiates Java wrapper contract tests."""
    dx_wrapper_contract_tests(
        name,
        file_classes = ["java"],
        extra_checks = [
            expect_equal(
                "empty gains werror and lint",
                java_javacopts_with_werror({})["javacopts"],
                ["-Werror", "-Xlint:all"],
            ),
            expect_equal(
                "existing werror is kept without duplication",
                java_javacopts_with_werror({"javacopts": ["-Werror"]})["javacopts"],
                ["-Werror", "-Xlint:all"],
            ),
            expect_equal(
                "existing lint gains werror",
                java_javacopts_with_werror({"javacopts": ["-Xlint:all"]})["javacopts"],
                ["-Xlint:all", "-Werror"],
            ),
            expect_equal(
                "both flags stay unchanged",
                java_javacopts_with_werror({"javacopts": ["-Werror", "-Xlint:all"]})["javacopts"],
                ["-Werror", "-Xlint:all"],
            ),
            expect_equal(
                "other kwargs survive",
                java_javacopts_with_werror({"deps": [":hello_lib"]})["deps"],
                [":hello_lib"],
            ),
            expect_equal(
                "maven test path infers class",
                java_entry_class("FooTest", ["src/test/java/com/x/FooTest.java"], "p"),
                "com.x.FooTest",
            ),
            expect_equal(
                "nested main path infers class",
                java_entry_class("M", ["src/main/java/hello/M.java"], "java/tests/fixtures/hello"),
                "hello.M",
            ),
            expect_equal(
                "basename mismatch uses package path",
                java_entry_class("Bar", ["Other.java"], "java/foo"),
                "foo.Bar",
            ),
            expect_equal(
                "path without root infers nothing",
                java_entry_class("X", ["X.java"], "p"),
                None,
            ),
            expect_equal(
                "colon label src infers class",
                java_entry_class("FooTest", [":src/test/java/com/x/FooTest.java"], "p"),
                "com.x.FooTest",
            ),
            expect_equal(
                "non-list srcs infer nothing",
                java_entry_class("FooTest", None, "java/foo"),
                None,
            ),
            expect_equal(
                "empty srcs use package path",
                java_entry_class("FooTest", [], "java/foo"),
                "foo.FooTest",
            ),
            expect_equal(
                "test gains inferred class",
                java_entry_kwargs("FooTest", ["src/test/java/com/x/FooTest.java"], "p", {}, "test_class"),
                {"test_class": "com.x.FooTest"},
            ),
            expect_equal(
                "explicit test class is kept",
                java_entry_kwargs("FooTest", ["src/test/java/com/x/FooTest.java"], "p", {"test_class": "a.B"}, "test_class")["test_class"],
                "a.B",
            ),
            expect_equal(
                "empty test class counts as unset",
                java_entry_kwargs("FooTest", ["src/test/java/com/x/FooTest.java"], "p", {"test_class": ""}, "test_class")["test_class"],
                "com.x.FooTest",
            ),
            expect_equal(
                "test without testrunner gains nothing",
                java_entry_kwargs("FooTest", ["src/test/java/com/x/FooTest.java"], "p", {"use_testrunner": False}, "test_class"),
                {"use_testrunner": False},
            ),
            expect_equal(
                "binary gains inferred main class",
                java_entry_kwargs("M", ["src/main/java/hello/M.java"], "java/x", {}, "main_class"),
                {"main_class": "hello.M"},
            ),
            expect_equal(
                "binary with testrunner gains nothing",
                java_entry_kwargs("M", ["src/main/java/hello/M.java"], "java/x", {"use_testrunner": True}, "main_class"),
                {"use_testrunner": True},
            ),
            expect_equal(
                "binary without executable gains nothing",
                java_entry_kwargs("M", ["src/main/java/hello/M.java"], "java/x", {"create_executable": False}, "main_class"),
                {"create_executable": False},
            ),
            expect_equal(
                "binary without srcs gains nothing",
                java_entry_kwargs("M", [], "java/x", {}, "main_class"),
                {},
            ),
            expect_equal(
                "binary without root gains nothing",
                java_entry_kwargs("M", ["M.java"], "p", {}, "main_class"),
                {},
            ),
            expect_equal(
                "javacopts survive inference",
                java_entry_kwargs("M", ["src/main/java/hello/M.java"], "java/x", java_javacopts_with_werror({}), "main_class")["javacopts"],
                ["-Werror", "-Xlint:all"],
            ),
        ],
    )
