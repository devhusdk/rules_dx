"""Identity tests pinning the hermetic C++ toolchain binding."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load(":bindings.bzl", "CC_TOOLCHAIN_TYPE", "cc_compiler_identity")

EXPECTED_IDENTITY_OBSERVATIONS = """subject //cc/toolchains:identity_under_test
field cc.abi=itanium
field cc.builtin_includes=external/llvm++glibc+glibc_headers_x86_64-linux-gnu.2.28/include | external/llvm++kernel_headers+linux_kernel_headers_x86.4.19.325/include | external/llvm++llvm+llvm-project/compiler-rt/include | external/llvm++llvm_toolchain_minimal+llvm-toolchain-minimal-linux-amd64/lib/clang/23 | external/llvm++llvm_toolchain_minimal+llvm-toolchain-minimal-linux-amd64/lib/clang/23/include
field cc.compiler=clang
field cc.coverage=llvm
field cc.driver=gnu
field cc.family=clang
field cc.syntax_only=-fsyntax-only
field cc.sysroot=None
field cc.target=x86_64-unknown-linux-gnu
aspect_field aspect_seen=True
aspect_field field_count=9
aspect_field has_subject=True
aspect_field subject_label=//cc/toolchains:identity_under_test
aspect_field transitive_count=0"""

_LINUX_X86_64 = ["@platforms//os:linux", "@platforms//cpu:x86_64"]

def cc_toolchain_identity_tests(name):
    starlark_test(
        name = name + "_binding",
        mode = "load",
        checks = [
            expect_equal(
                "cc binds to the C++ toolchain type",
                CC_TOOLCHAIN_TYPE,
                "@bazel_tools//tools/cpp:toolchain_type",
            ),
        ],
    )
    starlark_test(
        name = name + "_contract",
        mode = "load",
        checks = [
            expect_equal(
                "clang on x86_64-unknown-linux-gnu",
                cc_compiler_identity("clang", "x86_64-unknown-linux-gnu"),
                struct(family = "clang", driver = "gnu", abi = "itanium", coverage = "llvm", syntax_only = "-fsyntax-only"),
            ),
            expect_equal(
                "clang on x86_64-pc-windows-msvc",
                cc_compiler_identity("clang", "x86_64-pc-windows-msvc"),
                struct(family = "clang", driver = "gnu", abi = "msvc", coverage = "llvm", syntax_only = "-fsyntax-only"),
            ),
            expect_equal(
                "clang-cl on x86_64-pc-windows-msvc",
                cc_compiler_identity("clang-cl", "x86_64-pc-windows-msvc"),
                struct(family = "clang-cl", driver = "msvc", abi = "msvc", coverage = "llvm", syntax_only = "/Zs"),
            ),
            expect_equal(
                "msvc-cl on x86_64-pc-windows-msvc",
                cc_compiler_identity("msvc-cl", "x86_64-pc-windows-msvc"),
                struct(family = "msvc", driver = "msvc", abi = "msvc", coverage = "", syntax_only = "/Zs"),
            ),
            expect_equal(
                "msvc-cl on empty target",
                cc_compiler_identity("msvc-cl", ""),
                struct(family = "msvc", driver = "msvc", abi = "msvc", coverage = "", syntax_only = "/Zs"),
            ),
            expect_equal(
                "gcc on x86_64-linux-gnu",
                cc_compiler_identity("gcc", "x86_64-linux-gnu"),
                struct(family = "gcc", driver = "gnu", abi = "itanium", coverage = "gcov", syntax_only = "-fsyntax-only"),
            ),
            expect_equal(
                "mingw-gcc on x86_64-w64-mingw32",
                cc_compiler_identity("mingw-gcc", "x86_64-w64-mingw32"),
                struct(family = "gcc", driver = "gnu", abi = "itanium", coverage = "gcov", syntax_only = "-fsyntax-only"),
            ),
            expect_equal(
                "empty compiler on empty target",
                cc_compiler_identity("", ""),
                struct(family = "unknown", driver = "unknown", abi = "unknown", coverage = "", syntax_only = ""),
            ),
            expect_equal(
                "icx on x86_64-unknown-linux-gnu",
                cc_compiler_identity("icx", "x86_64-unknown-linux-gnu"),
                struct(family = "unknown", driver = "unknown", abi = "unknown", coverage = "", syntax_only = ""),
            ),
        ],
    )
    starlark_test(
        name = name,
        mode = "analysis",
        subjects = [":identity_under_test"],
        expected_observations = EXPECTED_IDENTITY_OBSERVATIONS,
        target_compatible_with = _LINUX_X86_64,
    )
