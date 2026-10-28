"""Tested-stack manifest contract tests."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")

def tested_stack_contract_tests(name):
    starlark_test(
        name = name,
        mode = "execution",
        checks = [
            expect_equal("contract has manifest evidence", 1, 1),
        ],
        file_checks = {
            "//:.bazelversion": "9.2.0",
            "//:MODULE.bazel": "name = \"rules_rust\", version = \"0.74.0\"\nname = \"rules_cc\", version = \"0.2.27\"\nname = \"rules_java\", version = \"9.9.0\"\nname = \"aspect_rules_js\", version = \"3.5.2\"\nversions = [\"1.98.0\"]",
            ":stack": "\"schema_version\": 2\n\"generator\": \"libs/testing/tested_stack.bzl\"\n\"bazel_version\": \"9.2.0\"\n\"rules_rust_version\": \"0.74.0\"\n\"rules_cc_version\": \"0.2.27\"\n\"rust_version\": \"1.98.0\"\n\"dotnet_version\": \"10.0.400\"\n\"go_sdk_version\": \"1.26.10\"\n\"pnpm_version\": \"10.34.5\"\n\"python_version\": \"3.12\"\n\"scala_version\": \"2.13.18\"\n\"typescript_version\": \"5.9.3\"\n\"module_deps\"\n\"rules_java\": \"9.9.0\"\n\"rules_dotnet\": \"0.22.2\"\n\"x86_64-unknown-linux-gnu\"",
        },
    )
