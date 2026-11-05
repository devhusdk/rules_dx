"""Tests for the MSVC layout and the toolchain macro."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load(":layout.bzl", "msvc_layout")
load(":subject.bzl", "msvc_toolchain_subject")
load(":toolchains.bzl", "msvc_toolchain_cells")

_ROOT = "cc\\tests\\fixtures\\msvc_layout\\"
_VC = "VC\\Tools\\MSVC\\14.44.35207\\"
_SDK = "Windows Kits\\10\\"

def _expected(label, cpu, target_system, machine, tool_dir, bin_dirs, lib_arch):
    """Returns the observations for a toolchain subject.

    Args:
        label: Subject target name.
        cpu: Expected cc_toolchain cpu.
        target_system: Expected target system name.
        machine: Expected /MACHINE flag.
        tool_dir: Slash-separated directory of the MSVC tools.
        bin_dirs: Backslash-separated PATH entries relative to the layout root.
        lib_arch: Target architecture directory name.

    Returns:
        The expected observation text.
    """
    sdk_include = _ROOT + _SDK + "Include\\10.0.26100.0\\"
    sdk_lib = _ROOT + _SDK + "Lib\\10.0.26100.0\\"
    tool = "cc/tests/fixtures/msvc_layout/VC/Tools/MSVC/14.44.35207/bin/" + tool_dir
    return "\n".join([
        "subject //cc/toolchains/msvc:" + label,
        "field cc.compiler=msvc-cl",
        "field cc.cpu=" + cpu,
        "field cc.family=msvc",
        "field cc.target_system=" + target_system,
        "field dll_link.has_dll_flag=True",
        "field env.INCLUDE=" + ";".join([_ROOT + _VC + "include", sdk_include + "ucrt", sdk_include + "shared", sdk_include + "um", sdk_include + "winrt"]),
        "field env.LIB=" + ";".join([_ROOT + _VC + "lib\\" + lib_arch, sdk_lib + "ucrt\\" + lib_arch, sdk_lib + "um\\" + lib_arch]),
        "field env.PATH=" + ";".join([_ROOT + d for d in bin_dirs]),
        "field env.TMP=C:\\Temp",
        "field flags.c_conformance=none",
        "field flags.cpp_conformance=/Zc:__cplusplus /permissive-",
        "field flags.dbg=/MDd",
        "field flags.default=/MD",
        "field flags.machine=" + machine,
        "field flags.static_link_msvcrt=/MT",
        "field tool.archive=" + tool + "/lib.exe",
        "field tool.c_compile=" + tool + "/cl.exe",
        "field tool.cpp_compile=" + tool + "/cl.exe",
        "field tool.dll_link=" + tool + "/link.exe",
        "field tool.exe_link=" + tool + "/link.exe",
        "aspect_field aspect_seen=True",
        "aspect_field field_count=20",
        "aspect_field has_subject=True",
        "aspect_field subject_label=//cc/toolchains/msvc:" + label,
        "aspect_field transitive_count=0",
    ])

_VC_BIN = _VC + "bin\\"
_SDK_BIN = _SDK + "bin\\10.0.26100.0\\"

CELLS = [
    struct(
        name = "x64_x64",
        toolchain = "toolchain",
        expected = _expected("msvc_x64_x64_under_test", "x64_windows", "x86_64-pc-windows-msvc", "/MACHINE:X64", "Hostx64/x64", [_VC_BIN + "Hostx64\\x64", _SDK_BIN + "x64"], "x64"),
    ),
    struct(
        name = "x64_arm64",
        toolchain = "toolchain_x64_arm64",
        expected = _expected("msvc_x64_arm64_under_test", "arm64_windows", "aarch64-pc-windows-msvc", "/MACHINE:ARM64", "Hostx64/arm64", [_VC_BIN + "Hostx64\\arm64", _VC_BIN + "Hostx64\\x64", _SDK_BIN + "x64"], "arm64"),
    ),
    struct(
        name = "arm64_arm64",
        toolchain = "toolchain_arm64_arm64",
        expected = _expected("msvc_arm64_arm64_under_test", "arm64_windows", "aarch64-pc-windows-msvc", "/MACHINE:ARM64", "Hostarm64/arm64", [_VC_BIN + "Hostarm64\\arm64", _SDK_BIN + "arm64"], "arm64"),
    ),
]

def msvc_toolchain_tests(name):
    """Declares the layout and per-cell toolchain tests.

    Args:
        name: Prefix of the declared tests.
    """
    layout = msvc_layout("14.44.35207", "10.0.26100.0")
    bin_dir = "VC/Tools/MSVC/14.44.35207/bin/Hostx64/x64"
    starlark_test(
        name = name + "_layout",
        mode = "load",
        checks = [
            expect_equal("cl", layout.cl, bin_dir + "/cl.exe"),
            expect_equal("link", layout.link, bin_dir + "/link.exe"),
            expect_equal("lib", layout.lib, bin_dir + "/lib.exe"),
            expect_equal("ml", layout.ml, bin_dir + "/ml64.exe"),
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
    cross = msvc_layout("14.44.35207", "10.0.26100.0", "x64", "arm64")
    native = msvc_layout("14.44.35207", "10.0.26100.0", "arm64", "arm64")
    vc = "VC/Tools/MSVC/14.44.35207/bin/"
    starlark_test(
        name = name + "_layout_arm64",
        mode = "load",
        checks = [
            expect_equal("cross cl", cross.cl, vc + "Hostx64/arm64/cl.exe"),
            expect_equal("cross ml", cross.ml, None),
            expect_equal("cross lib dirs", cross.lib_dirs[0], "VC/Tools/MSVC/14.44.35207/lib/arm64"),
            expect_equal("cross bin dirs", cross.bin_dirs, [vc + "Hostx64/arm64", vc + "Hostx64/x64", "Windows Kits/10/bin/10.0.26100.0/x64"]),
            expect_equal("native cl", native.cl, vc + "Hostarm64/arm64/cl.exe"),
            expect_equal("native bin dirs", native.bin_dirs, [vc + "Hostarm64/arm64", "Windows Kits/10/bin/10.0.26100.0/arm64"]),
        ],
    )
    x64 = "@platforms//cpu:x86_64"
    arm64 = "@platforms//cpu:arm64"
    starlark_test(
        name = name + "_toolchain_cells",
        mode = "load",
        checks = [
            expect_equal("cells", msvc_toolchain_cells(), [
                ("msvc_windows_x86_64", x64, x64),
                ("msvc_windows_x86_64_to_arm64", x64, arm64),
                ("msvc_windows_arm64", arm64, arm64),
            ]),
        ],
    )
    for cell in CELLS:
        subject = name + "_" + cell.name + "_under_test"
        msvc_toolchain_subject(
            name = subject,
            toolchain = "//cc/tests/fixtures/msvc_layout:" + cell.toolchain,
        )
        starlark_test(
            name = name + "_" + cell.name + "_toolchain",
            mode = "analysis",
            subjects = [":" + subject],
            expected_observations = cell.expected,
        )
