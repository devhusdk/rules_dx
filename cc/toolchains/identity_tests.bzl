"""Identity tests pinning the hermetic C++ toolchain binding."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load(":bindings.bzl", "CC_TOOLCHAIN_TYPE", "cc_compiler_identity", "cc_toolchain_identity")
load(":pins.bzl", "LLVM_CLANG_VERSION")

_CLANG_MAJOR = LLVM_CLANG_VERSION.split(".")[0]

EXPECTED_IDENTITY_OBSERVATIONS = """subject //cc/toolchains:identity_under_test
field cc.abi=itanium
field cc.builtin_includes=external/llvm++glibc+glibc_headers_x86_64-linux-gnu.2.28/include | external/llvm++kernel_headers+linux_kernel_headers_x86.4.19.325/include | external/llvm++llvm+llvm-project/compiler-rt/include | external/llvm++llvm_toolchain_minimal+llvm-toolchain-minimal-linux-amd64/lib/clang/%s | external/llvm++llvm_toolchain_minimal+llvm-toolchain-minimal-linux-amd64/lib/clang/%s/include
field cc.compiler=clang
field cc.coverage=llvm
field cc.driver=gnu
field cc.family=clang
field cc.libc=glibc
field cc.syntax_only=-fsyntax-only
field cc.sysroot=None
field cc.target=x86_64-unknown-linux-gnu
aspect_field aspect_seen=True
aspect_field field_count=10
aspect_field has_subject=True
aspect_field subject_label=//cc/toolchains:identity_under_test
aspect_field transitive_count=0""" % (_CLANG_MAJOR, _CLANG_MAJOR)

EXPECTED_GCC_IDENTITY_OBSERVATIONS = """subject //cc/toolchains:identity_under_test_gcc
field cc.abi=itanium
field cc.builtin_includes=external/+gcc+dx_gcc_linux_x86_64_16_2_0/external/+gcc+dx_gcc_linux_x86_64_16_2_0/include/c++/16.2.0 | external/+gcc+dx_gcc_linux_x86_64_16_2_0/external/+gcc+dx_gcc_linux_x86_64_16_2_0/include/c++/16.2.0/backward | external/+gcc+dx_gcc_linux_x86_64_16_2_0/external/+gcc+dx_gcc_linux_x86_64_16_2_0/include/c++/16.2.0/x86_64-linux | external/+gcc+dx_gcc_linux_x86_64_16_2_0/external/+gcc+dx_gcc_linux_x86_64_16_2_0/lib/gcc/x86_64-linux/16.2.0/include | external/+gcc+dx_gcc_linux_x86_64_16_2_0/external/+gcc+dx_gcc_linux_x86_64_16_2_0/lib/gcc/x86_64-linux/16.2.0/include-fixed | external/+gcc+dx_gcc_linux_x86_64_16_2_0/external/+gcc+dx_gcc_linux_x86_64_16_2_0/sysroot/usr/include
field cc.compiler=gcc
field cc.coverage=gcov
field cc.driver=gnu
field cc.family=gcc
field cc.libc=glibc_2.28
field cc.syntax_only=-fsyntax-only
field cc.sysroot=None
field cc.target=x86_64-linux-gnu
aspect_field aspect_seen=True
aspect_field field_count=10
aspect_field has_subject=True
aspect_field subject_label=//cc/toolchains:identity_under_test_gcc
aspect_field transitive_count=0"""

EXPECTED_GCC_ARM64_IDENTITY_OBSERVATIONS = """subject //cc/toolchains:identity_under_test_gcc_arm64
field cc.abi=itanium
field cc.builtin_includes=external/+gcc+dx_gcc_linux_aarch64_cross_16_2_0/external/+gcc+dx_gcc_linux_aarch64_cross_16_2_0/aarch64-linux/include | external/+gcc+dx_gcc_linux_aarch64_cross_16_2_0/external/+gcc+dx_gcc_linux_aarch64_cross_16_2_0/aarch64-linux/include/c++/16.2.0 | external/+gcc+dx_gcc_linux_aarch64_cross_16_2_0/external/+gcc+dx_gcc_linux_aarch64_cross_16_2_0/aarch64-linux/include/c++/16.2.0/aarch64-linux | external/+gcc+dx_gcc_linux_aarch64_cross_16_2_0/external/+gcc+dx_gcc_linux_aarch64_cross_16_2_0/aarch64-linux/include/c++/16.2.0/backward | external/+gcc+dx_gcc_linux_aarch64_cross_16_2_0/external/+gcc+dx_gcc_linux_aarch64_cross_16_2_0/lib/gcc/aarch64-linux/16.2.0/include | external/+gcc+dx_gcc_linux_aarch64_cross_16_2_0/external/+gcc+dx_gcc_linux_aarch64_cross_16_2_0/lib/gcc/aarch64-linux/16.2.0/include-fixed | external/+gcc+dx_gcc_linux_aarch64_cross_16_2_0/external/+gcc+dx_gcc_linux_aarch64_cross_16_2_0/sysroot/usr/include
field cc.compiler=gcc
field cc.coverage=gcov
field cc.driver=gnu
field cc.family=gcc
field cc.libc=glibc_2.28
field cc.syntax_only=-fsyntax-only
field cc.sysroot=None
field cc.target=aarch64-linux-gnu
aspect_field aspect_seen=True
aspect_field field_count=10
aspect_field has_subject=True
aspect_field subject_label=//cc/toolchains:identity_under_test_gcc_arm64
aspect_field transitive_count=0"""

EXPECTED_GCC_ARM64_NATIVE_IDENTITY_OBSERVATIONS = """subject //cc/toolchains:identity_under_test_gcc_arm64_native
field cc.abi=itanium
field cc.builtin_includes=external/+gcc+dx_gcc_linux_aarch64_16_2_0/external/+gcc+dx_gcc_linux_aarch64_16_2_0/aarch64-linux/include | external/+gcc+dx_gcc_linux_aarch64_16_2_0/external/+gcc+dx_gcc_linux_aarch64_16_2_0/aarch64-linux/include/c++/16.2.0 | external/+gcc+dx_gcc_linux_aarch64_16_2_0/external/+gcc+dx_gcc_linux_aarch64_16_2_0/aarch64-linux/include/c++/16.2.0/aarch64-linux | external/+gcc+dx_gcc_linux_aarch64_16_2_0/external/+gcc+dx_gcc_linux_aarch64_16_2_0/aarch64-linux/include/c++/16.2.0/backward | external/+gcc+dx_gcc_linux_aarch64_16_2_0/external/+gcc+dx_gcc_linux_aarch64_16_2_0/lib/gcc/aarch64-linux/16.2.0/include | external/+gcc+dx_gcc_linux_aarch64_16_2_0/external/+gcc+dx_gcc_linux_aarch64_16_2_0/lib/gcc/aarch64-linux/16.2.0/include-fixed | external/+gcc+dx_gcc_linux_aarch64_16_2_0/external/+gcc+dx_gcc_linux_aarch64_16_2_0/sysroot/usr/include
field cc.compiler=gcc
field cc.coverage=gcov
field cc.driver=gnu
field cc.family=gcc
field cc.libc=glibc_2.28
field cc.syntax_only=-fsyntax-only
field cc.sysroot=None
field cc.target=aarch64-linux-gnu
aspect_field aspect_seen=True
aspect_field field_count=10
aspect_field has_subject=True
aspect_field subject_label=//cc/toolchains:identity_under_test_gcc_arm64_native
aspect_field transitive_count=0"""

EXPECTED_MSVC_IDENTITY_OBSERVATIONS = """subject //cc/toolchains:identity_under_test_msvc
field cc.abi=msvc
field cc.builtin_includes=external/+msvc+dx_msvc/VC/Tools/MSVC/14.50.35717/include | external/+msvc+dx_msvc/Windows Kits/10/Include/10.0.26100.0/shared | external/+msvc+dx_msvc/Windows Kits/10/Include/10.0.26100.0/ucrt | external/+msvc+dx_msvc/Windows Kits/10/Include/10.0.26100.0/um | external/+msvc+dx_msvc/Windows Kits/10/Include/10.0.26100.0/winrt
field cc.compiler=msvc-cl
field cc.coverage=
field cc.driver=msvc
field cc.family=msvc
field cc.libc=ucrt
field cc.syntax_only=/Zs
field cc.sysroot=None
field cc.target=x86_64-pc-windows-msvc
field cc.version=14.50.35717
aspect_field aspect_seen=True
aspect_field field_count=11
aspect_field has_subject=True
aspect_field subject_label=//cc/toolchains:identity_under_test_msvc
aspect_field transitive_count=0"""

EXPECTED_MSVC_ARM64_IDENTITY_OBSERVATIONS = (
    EXPECTED_MSVC_IDENTITY_OBSERVATIONS
        .replace("identity_under_test_msvc", "identity_under_test_msvc_arm64")
        .replace("cc.target=x86_64-pc-windows-msvc", "cc.target=aarch64-pc-windows-msvc")
)

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
                struct(family = "clang", driver = "gnu", abi = "itanium", coverage = "llvm", syntax_only = "-fsyntax-only", target = "x86_64-unknown-linux-gnu"),
            ),
            expect_equal(
                "clang on x86_64-pc-windows-msvc",
                cc_compiler_identity("clang", "x86_64-pc-windows-msvc"),
                struct(family = "clang", driver = "gnu", abi = "msvc", coverage = "llvm", syntax_only = "-fsyntax-only", target = "x86_64-pc-windows-msvc"),
            ),
            expect_equal(
                "clang-cl on x86_64-pc-windows-msvc",
                cc_compiler_identity("clang-cl", "x86_64-pc-windows-msvc"),
                struct(family = "clang-cl", driver = "msvc", abi = "msvc", coverage = "llvm", syntax_only = "/Zs", target = "x86_64-pc-windows-msvc"),
            ),
            expect_equal(
                "msvc-cl on x86_64-pc-windows-msvc",
                cc_compiler_identity("msvc-cl", "x86_64-pc-windows-msvc"),
                struct(family = "msvc", driver = "msvc", abi = "msvc", coverage = "", syntax_only = "/Zs", target = "x86_64-pc-windows-msvc"),
            ),
            expect_equal(
                "msvc-cl on empty target",
                cc_compiler_identity("msvc-cl", ""),
                struct(family = "msvc", driver = "msvc", abi = "msvc", coverage = "", syntax_only = "/Zs", target = ""),
            ),
            expect_equal(
                "gcc on x86_64-linux-gnu",
                cc_compiler_identity("gcc", "x86_64-linux-gnu"),
                struct(family = "gcc", driver = "gnu", abi = "itanium", coverage = "gcov", syntax_only = "-fsyntax-only", target = "x86_64-linux-gnu"),
            ),
            expect_equal(
                "mingw-gcc on x86_64-w64-mingw32",
                cc_compiler_identity("mingw-gcc", "x86_64-w64-mingw32"),
                struct(family = "gcc", driver = "gnu", abi = "itanium", coverage = "gcov", syntax_only = "-fsyntax-only", target = "x86_64-w64-mingw32"),
            ),
            expect_equal(
                "empty compiler on empty target",
                cc_compiler_identity("", ""),
                struct(family = "unknown", driver = "unknown", abi = "unknown", coverage = "", syntax_only = "", target = ""),
            ),
            expect_equal(
                "no resolved toolchain gives the unknown identity",
                cc_toolchain_identity(struct(toolchains = {CC_TOOLCHAIN_TYPE: None})),
                cc_compiler_identity("", ""),
            ),
            expect_equal(
                "icx on x86_64-unknown-linux-gnu",
                cc_compiler_identity("icx", "x86_64-unknown-linux-gnu"),
                struct(family = "unknown", driver = "unknown", abi = "unknown", coverage = "", syntax_only = "", target = "x86_64-unknown-linux-gnu"),
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
    starlark_test(
        name = name + "_gcc",
        mode = "analysis",
        subjects = [":identity_under_test_gcc"],
        expected_observations = EXPECTED_GCC_IDENTITY_OBSERVATIONS,
        target_compatible_with = _LINUX_X86_64,
    )
    starlark_test(
        name = name + "_gcc_arm64",
        mode = "analysis",
        subjects = [":identity_under_test_gcc_arm64"],
        expected_observations = EXPECTED_GCC_ARM64_IDENTITY_OBSERVATIONS,
        target_compatible_with = _LINUX_X86_64,
    )
    starlark_test(
        name = name + "_gcc_arm64_native",
        mode = "analysis",
        subjects = [":identity_under_test_gcc_arm64_native"],
        expected_observations = EXPECTED_GCC_ARM64_NATIVE_IDENTITY_OBSERVATIONS,
        target_compatible_with = ["@platforms//os:linux", "@platforms//cpu:arm64"],
    )
    starlark_test(
        name = name + "_msvc",
        mode = "analysis",
        subjects = [":identity_under_test_msvc"],
        expected_observations = EXPECTED_MSVC_IDENTITY_OBSERVATIONS,
        target_compatible_with = ["@platforms//os:windows", "@platforms//cpu:x86_64"],
    )
    starlark_test(
        name = name + "_msvc_arm64",
        mode = "analysis",
        subjects = [":identity_under_test_msvc_arm64"],
        expected_observations = EXPECTED_MSVC_ARM64_IDENTITY_OBSERVATIONS,
        target_compatible_with = ["@platforms//os:windows"],
    )
