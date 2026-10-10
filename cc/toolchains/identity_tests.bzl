"""Identity tests pinning the hermetic C++ toolchain binding."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load(":bindings.bzl", "CC_TOOLCHAIN_TYPE")

EXPECTED_IDENTITY_OBSERVATIONS = """subject //cc/toolchains:identity_under_test
field cc.builtin_includes=external/llvm++glibc+glibc_headers_x86_64-linux-gnu.2.28/include | external/llvm++kernel_headers+linux_kernel_headers_x86.4.19.325/include | external/llvm++llvm+llvm-project/compiler-rt/include | external/llvm++llvm_toolchain_minimal+llvm-toolchain-minimal-linux-amd64/lib/clang/23
field cc.compiler=clang
field cc.sysroot=None
field cc.target=x86_64-unknown-linux-gnu
aspect_field aspect_seen=True
aspect_field field_count=4
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
        name = name,
        mode = "analysis",
        subjects = [":identity_under_test"],
        expected_observations = EXPECTED_IDENTITY_OBSERVATIONS,
        target_compatible_with = _LINUX_X86_64,
    )
