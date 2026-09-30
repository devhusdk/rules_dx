"""Wrapper conformance tests for C/C++."""

load("//libs/starlark:conformance.bzl", "dx_wrapper_contract_tests")
load("//libs/starlark:defs.bzl", "expect_equal", "expect_match", "expect_true")
load(":defs.bzl", "cc_copts_with_werror")

def cc_wrapper_contract_tests(name):
    """Instantiates C/C++ wrapper contract tests."""
    dx_wrapper_contract_tests(
        name,
        file_classes = ["c", "cpp", "cuda"],
        extra_checks = [
            expect_true(
                "empty gains platform werror select",
                type(cc_copts_with_werror({})["copts"]) == "select",
            ),
            expect_match(
                "empty select covers windows WX and default Werror",
                str(cc_copts_with_werror({})["copts"]),
                "windows",
            ),
            expect_match(
                "macos disables profile generation",
                str(cc_copts_with_werror({})["copts"]),
                "macos",
            ),
            expect_match(
                "macos select carries fno-profile-instr-generate",
                str(cc_copts_with_werror({})["copts"]),
                "fno-profile-instr-generate",
            ),
            expect_equal(
                "existing werror is kept without duplication",
                cc_copts_with_werror({"copts": ["-Werror"]})["copts"],
                ["-Werror"],
            ),
            expect_equal(
                "existing msvc werror is kept without duplication",
                cc_copts_with_werror({"copts": ["/WX"]})["copts"],
                ["/WX"],
            ),
            expect_true(
                "other flags stay under the platform select",
                type(cc_copts_with_werror({"copts": ["-Wall"]})["copts"]) == "select",
            ),
            expect_match(
                "other flags keep -Wall in the select",
                str(cc_copts_with_werror({"copts": ["-Wall"]})["copts"]),
                "-Wall",
            ),
            expect_match(
                "windows std rewrite carries /Zc:__cplusplus",
                str(cc_copts_with_werror({"copts": ["-std=c++17"]})["copts"]),
                "/Zc:__cplusplus",
            ),
            expect_equal(
                "other kwargs survive",
                cc_copts_with_werror({"deps": [":hello_lib"]})["deps"],
                [":hello_lib"],
            ),
        ],
    )
