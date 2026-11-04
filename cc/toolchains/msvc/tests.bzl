"""Tests for the MSVC layout and the toolchain macro."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load(":layout.bzl", "msvc_layout")
load(":subject.bzl", "msvc_toolchain_subject")

EXPECTED_OBSERVATIONS = """subject //cc/toolchains/msvc:msvc_under_test
field cc.compiler=msvc-cl
field cc.family=msvc
field dll_link.has_dll_flag=True
field env.INCLUDE=cc\\tests\\fixtures\\msvc_layout\\VC\\Tools\\MSVC\\14.44.35207\\include;cc\\tests\\fixtures\\msvc_layout\\Windows Kits\\10\\Include\\10.0.26100.0\\ucrt;cc\\tests\\fixtures\\msvc_layout\\Windows Kits\\10\\Include\\10.0.26100.0\\shared;cc\\tests\\fixtures\\msvc_layout\\Windows Kits\\10\\Include\\10.0.26100.0\\um;cc\\tests\\fixtures\\msvc_layout\\Windows Kits\\10\\Include\\10.0.26100.0\\winrt
field env.LIB=cc\\tests\\fixtures\\msvc_layout\\VC\\Tools\\MSVC\\14.44.35207\\lib\\x64;cc\\tests\\fixtures\\msvc_layout\\Windows Kits\\10\\Lib\\10.0.26100.0\\ucrt\\x64;cc\\tests\\fixtures\\msvc_layout\\Windows Kits\\10\\Lib\\10.0.26100.0\\um\\x64
field env.PATH=cc\\tests\\fixtures\\msvc_layout\\VC\\Tools\\MSVC\\14.44.35207\\bin\\Hostx64\\x64;cc\\tests\\fixtures\\msvc_layout\\Windows Kits\\10\\bin\\10.0.26100.0\\x64
field env.TMP=C:\\Temp
field flags.c_conformance=none
field flags.cpp_conformance=/Zc:__cplusplus /permissive-
field flags.dbg=/MDd
field flags.default=/MD
field flags.static_link_msvcrt=/MT
field tool.archive=cc/tests/fixtures/msvc_layout/VC/Tools/MSVC/14.44.35207/bin/Hostx64/x64/lib.exe
field tool.c_compile=cc/tests/fixtures/msvc_layout/VC/Tools/MSVC/14.44.35207/bin/Hostx64/x64/cl.exe
field tool.cpp_compile=cc/tests/fixtures/msvc_layout/VC/Tools/MSVC/14.44.35207/bin/Hostx64/x64/cl.exe
field tool.dll_link=cc/tests/fixtures/msvc_layout/VC/Tools/MSVC/14.44.35207/bin/Hostx64/x64/link.exe
field tool.exe_link=cc/tests/fixtures/msvc_layout/VC/Tools/MSVC/14.44.35207/bin/Hostx64/x64/link.exe
aspect_field aspect_seen=True
aspect_field field_count=17
aspect_field has_subject=True
aspect_field subject_label=//cc/toolchains/msvc:msvc_under_test
aspect_field transitive_count=0"""

def msvc_toolchain_tests(name):
    layout = msvc_layout("14.44.35207", "10.0.26100.0")
    bin_dir = "VC/Tools/MSVC/14.44.35207/bin/Hostx64/x64"
    starlark_test(
        name = name + "_layout",
        mode = "load",
        checks = [
            expect_equal("cl", layout.cl, bin_dir + "/cl.exe"),
            expect_equal("link", layout.link, bin_dir + "/link.exe"),
            expect_equal("lib", layout.lib, bin_dir + "/lib.exe"),
            expect_equal("ml64", layout.ml64, bin_dir + "/ml64.exe"),
            expect_equal("include dirs", layout.include_dirs, [
                "VC/Tools/MSVC/14.44.35207/include",
                "Windows Kits/10/Include/10.0.26100.0/ucrt",
                "Windows Kits/10/Include/10.0.26100.0/shared",
                "Windows Kits/10/Include/10.0.26100.0/um",
                "Windows Kits/10/Include/10.0.26100.0/winrt",
            ]),
            expect_equal("lib dirs", layout.lib_dirs, [
                "VC/Tools/MSVC/14.44.35207/lib/x64",
                "Windows Kits/10/Lib/10.0.26100.0/ucrt/x64",
                "Windows Kits/10/Lib/10.0.26100.0/um/x64",
            ]),
            expect_equal("bin dirs", layout.bin_dirs, [bin_dir, "Windows Kits/10/bin/10.0.26100.0/x64"]),
        ],
    )
    msvc_toolchain_subject(
        name = name + "_under_test",
        toolchain = "//cc/tests/fixtures/msvc_layout:toolchain",
    )
    starlark_test(
        name = name + "_toolchain",
        mode = "analysis",
        subjects = [":" + name + "_under_test"],
        expected_observations = EXPECTED_OBSERVATIONS,
    )
