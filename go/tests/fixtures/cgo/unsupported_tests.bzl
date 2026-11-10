"""Analysis tests that cgo rejects C++ toolchains it cannot use."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")

_WINDOWS = "x86_64-pc-windows-msvc"

_CASES = {
    "clang_cl": (
        "//cc/private/toolchains/external:clang_cl_toolchain",
        "cgo needs a GCC/Clang-style driver, but the C++ toolchain is clang-cl for " + _WINDOWS + ".",
    ),
    "clang_msvc": (
        "//cc/private/toolchains/external:clang_msvc_toolchain",
        "Go cgo on Windows needs the MinGW (GNU) ABI, but the C++ toolchain is clang for " + _WINDOWS + ".",
    ),
    "msvc_cl": (
        "//cc/private/toolchains/external:msvc_cl_toolchain",
        "cgo needs a GCC/Clang-style driver, but the C++ toolchain is msvc-cl for " + _WINDOWS + ".",
    ),
}

def _impl(ctx):
    env = analysistest.begin(ctx)
    asserts.expect_failure(env, ctx.attr.message)
    return analysistest.end(env)

def _make(case):
    return analysistest.make(
        _impl,
        attrs = {"message": attr.string(mandatory = True)},
        expect_failure = True,
        config_settings = {
            "//command_line_option:extra_toolchains": [str(Label(_CASES[case][0]))],
        },
    )

_clang_cl_test = _make("clang_cl")
_clang_msvc_test = _make("clang_msvc")
_msvc_cl_test = _make("msvc_cl")

_RULES = {
    "clang_cl": _clang_cl_test,
    "clang_msvc": _clang_msvc_test,
    "msvc_cl": _msvc_cl_test,
}

def cgo_unsupported_toolchain_tests(name, target_under_test):
    """Checks that cgo fails analysis on MSVC-driver and MSVC-ABI toolchains.

    Args:
      name: Name of the test suite.
      target_under_test: A go_library with cgo enabled.
    """
    tests = []
    for case, (_, message) in _CASES.items():
        test = name + "_" + case
        _RULES[case](
            name = test,
            message = message,
            size = "small",
            target_under_test = target_under_test,
        )
        tests.append(":" + test)
    native.test_suite(name = name, tests = tests)
