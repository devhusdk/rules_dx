"""Tested-stack manifest contract tests."""

load("//core:defs.bzl", "expect_equal", "starlark_test")
load("//core/modules:versions.bzl", "ASPECT_RULES_JS_VERSION", "DOTNET_VERSION", "GO_SDK_VERSION", "PNPM_VERSION", "PYTHON_VERSION", "RULES_CC_VERSION", "RULES_DOTNET_VERSION", "RULES_JAVA_VERSION", "RULES_RUST_VERSION", "RUST_VERSION", "SCALA_VERSION", "TYPESCRIPT_VERSION")

def _lines(lines):
    return "\n".join(lines)

def tested_stack_contract_tests(name):
    starlark_test(
        name = name,
        mode = "execution",
        checks = [
            expect_equal("contract has manifest evidence", 1, 1),
        ],
        file_checks = {
            "//:MODULE.bazel": _lines([
                'name = "rules_rust", version = "%s"' % RULES_RUST_VERSION,
                'name = "rules_cc", version = "%s"' % RULES_CC_VERSION,
                'name = "rules_java", version = "%s"' % RULES_JAVA_VERSION,
                'name = "aspect_rules_js", version = "%s"' % ASPECT_RULES_JS_VERSION,
                'versions = ["%s"]' % RUST_VERSION,
            ]),
            ":stack": _lines([
                '"schema_version": 2',
                '"generator": "libs/testing/tested_stack.bzl"',
                '"rules_rust_version": "%s"' % RULES_RUST_VERSION,
                '"rules_cc_version": "%s"' % RULES_CC_VERSION,
                '"rust_version": "%s"' % RUST_VERSION,
                '"dotnet_version": "%s"' % DOTNET_VERSION,
                '"go_sdk_version": "%s"' % GO_SDK_VERSION,
                '"pnpm_version": "%s"' % PNPM_VERSION,
                '"python_version": "%s"' % PYTHON_VERSION,
                '"scala_version": "%s"' % SCALA_VERSION,
                '"typescript_version": "%s"' % TYPESCRIPT_VERSION,
                '"module_deps"',
                '"rules_java": "%s"' % RULES_JAVA_VERSION,
                '"rules_dotnet": "%s"' % RULES_DOTNET_VERSION,
                '"x86_64-unknown-linux-gnu"',
            ]),
        },
    )
