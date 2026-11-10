"""Identity test for the consumer-declared GCC distribution."""

load("//core:defs.bzl", "starlark_test")

EXPECTED_OBSERVATIONS = """subject //cc/toolchains/external:custom_gcc_subject
field cc.abi=itanium
field cc.builtin_includes=external/gcc_toolchain++gcc_toolchains+custom_gcc_x86_64_16_2_0/external/gcc_toolchain++gcc_toolchains+custom_gcc_x86_64_16_2_0/include/c++/16.2.0 | external/gcc_toolchain++gcc_toolchains+custom_gcc_x86_64_16_2_0/external/gcc_toolchain++gcc_toolchains+custom_gcc_x86_64_16_2_0/include/c++/16.2.0/backward | external/gcc_toolchain++gcc_toolchains+custom_gcc_x86_64_16_2_0/external/gcc_toolchain++gcc_toolchains+custom_gcc_x86_64_16_2_0/include/c++/16.2.0/x86_64-linux | external/gcc_toolchain++gcc_toolchains+custom_gcc_x86_64_16_2_0/external/gcc_toolchain++gcc_toolchains+custom_gcc_x86_64_16_2_0/lib/gcc/x86_64-linux/16.2.0/finclude | external/gcc_toolchain++gcc_toolchains+custom_gcc_x86_64_16_2_0/external/gcc_toolchain++gcc_toolchains+custom_gcc_x86_64_16_2_0/lib/gcc/x86_64-linux/16.2.0/include | external/gcc_toolchain++gcc_toolchains+custom_gcc_x86_64_16_2_0/external/gcc_toolchain++gcc_toolchains+custom_gcc_x86_64_16_2_0/lib/gcc/x86_64-linux/16.2.0/include-fixed | external/gcc_toolchain++gcc_toolchains+custom_gcc_x86_64_16_2_0/external/gcc_toolchain++gcc_toolchains+custom_gcc_x86_64_16_2_0/sysroot/usr/include
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
aspect_field subject_label=//cc/toolchains/external:custom_gcc_subject
aspect_field transitive_count=0"""

def custom_gcc_identity_test(name):
    starlark_test(
        name = name,
        mode = "analysis",
        subjects = [":custom_gcc_subject"],
        expected_observations = EXPECTED_OBSERVATIONS,
        target_compatible_with = ["@platforms//os:linux", "@platforms//cpu:x86_64"],
    )
