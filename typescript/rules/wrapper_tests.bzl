"""Wrapper-contract tests for the TypeScript wrappers (item 2)."""

load("//libs/starlark:conformance.bzl", "dx_wrapper_contract_tests")
load("//libs/starlark:defs.bzl", "expect_equal")
load(":defs.bzl", "dx_tsconfig_is_source", "dx_tsconfig_relpath", "typescript_srcs_rejection", "typescript_test_env", "typescript_test_rejection")

def typescript_wrapper_contract_tests(name):
    """Instantiates TypeScript wrapper contract tests."""
    dx_wrapper_contract_tests(
        name,
        include_forwarder_checks = False,
        extra_checks = [
            expect_equal(
                "declaration file is rejected",
                typescript_srcs_rejection(["types.d.ts"]) != None,
                True,
            ),
            expect_equal(
                "declaration rejection names the files",
                typescript_srcs_rejection(["types.d.ts"]),
                "typescript_project takes real sources only; declaration " +
                "files are inert and must not be listed in srcs: types.d.ts",
            ),
            expect_equal(
                "every declaration suffix is rejected",
                typescript_srcs_rejection(["a.d.ts", "b.d.mts", "c.d.cts"]) != None,
                True,
            ),
            expect_equal(
                "rejected files render sorted",
                typescript_srcs_rejection(["b.d.mts", "a.d.ts"]),
                "typescript_project takes real sources only; declaration " +
                "files are inert and must not be listed in srcs: a.d.ts, b.d.mts",
            ),
            expect_equal(
                "mixed srcs reject only the declarations",
                typescript_srcs_rejection(["main.ts", "types.d.ts"]),
                "typescript_project takes real sources only; declaration " +
                "files are inert and must not be listed in srcs: types.d.ts",
            ),
            expect_equal(
                "real sources are clean",
                typescript_srcs_rejection(["main.ts", "view.tsx", "lib.mts", "old.cts"]),
                None,
            ),
            expect_equal(
                "similar non-declaration names are clean",
                typescript_srcs_rejection(["d.ts", "ad.ts", "types.d.ts.bak"]),
                None,
            ),
            expect_equal(
                "empty srcs are clean",
                typescript_srcs_rejection([]),
                None,
            ),
            expect_equal(
                "disabling standard reporters is rejected",
                typescript_test_rejection({"auto_configure_reporters": False}) != None,
                True,
            ),
            expect_equal(
                "reporter rejection names the standard logs",
                typescript_test_rejection({"auto_configure_reporters": False}),
                "typescript_test always uses jest with the standard " +
                "auto-configured reporters (Bazel test logs); " +
                "`auto_configure_reporters = False` is not supported.",
            ),
            expect_equal(
                "empty kwargs are clean",
                typescript_test_rejection({}),
                None,
            ),
            expect_equal(
                "explicit standard reporters are clean",
                typescript_test_rejection({"auto_configure_reporters": True}),
                None,
            ),
            expect_equal(
                "ordinary jest kwargs are clean",
                typescript_test_rejection({"config": "jest.config.cjs", "snapshots": False}),
                None,
            ),
            expect_equal(
                "missing env gains the filter channel",
                typescript_test_env(None),
                ["TESTBRIDGE_TEST_ONLY"],
            ),
            expect_equal(
                "caller env keeps entries and gains the filter channel",
                typescript_test_env(["FOO"]),
                ["FOO", "TESTBRIDGE_TEST_ONLY"],
            ),
            expect_equal(
                "present filter channel is not duplicated",
                typescript_test_env(["FOO", "TESTBRIDGE_TEST_ONLY"]),
                ["FOO", "TESTBRIDGE_TEST_ONLY"],
            ),
            expect_equal(
                "same-directory inherited config takes an explicit prefix",
                dx_tsconfig_relpath("typescript/tests/fixtures/entries/tsconfig_helper.json", "typescript/tests/fixtures/entries/tsconfig.json"),
                "./tsconfig.json",
            ),
            expect_equal(
                "parent-directory inherited config navigates up",
                dx_tsconfig_relpath("a/b/out.json", "a/tsconfig.json"),
                "../tsconfig.json",
            ),
            expect_equal(
                "root-level files take an explicit prefix",
                dx_tsconfig_relpath("out.json", "other.json"),
                "./other.json",
            ),
            expect_equal(
                "other-repository inherited config navigates to the root",
                dx_tsconfig_relpath("pkg/out.json", "../ext/pkg/tsconfig.json"),
                "../../ext/pkg/tsconfig.json",
            ),
            expect_equal(
                "every TypeScript extension is listed",
                [dx_tsconfig_is_source(s) for s in ["main.ts", "view.tsx", "lib.mts", "old.cts"]],
                [True, True, True, True],
            ),
            expect_equal(
                "declaration files stay listed like the upstream default",
                dx_tsconfig_is_source("types.d.ts"),
                True,
            ),
            expect_equal(
                "other extensions are not listed",
                [dx_tsconfig_is_source(s) for s in ["main.js", "data.json", "README.md"]],
                [False, False, False],
            ),
        ],
    )
