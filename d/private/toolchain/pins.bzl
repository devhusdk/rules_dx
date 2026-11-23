"""Pinned LDC compiler and rules_d identities; the archive data is in pins.json."""

load("@dx_pins_data//:pins.bzl", "DATA")

_PINS = DATA["dx_d"]

_ARCHIVES = _PINS["archives"]

RULES_D_VERSION = _ARCHIVES["rules_d"]["version"]

LDC_VERSION = _PINS["version"]

MSVC_ABI = "@llvm//constraints/windows/abi:msvc"

_WINDOWS_LINKOPTS = ["/NODEFAULTLIB:legacy_stdio_definitions.lib"]

_WINDOWS_ADDON = struct(
    archive = _ARCHIVES["ldc_windows_addon"]["urls"][0].rpartition("/")[2],
    keep = ["etc/ldc2.conf/55-target-windows-arm64.conf", "lib-windows-arm64"],
    sha256 = _ARCHIVES["ldc_windows_addon"]["sha256"],
    urls = _ARCHIVES["ldc_windows_addon"]["urls"],
)

def _cell(cell):
    return _PINS["cells"][cell]

LDC_CELLS = {
    "linux_x86_64": struct(
        addon = None,
        archive = _cell("linux_x86_64")["urls"][0].rpartition("/")[2],
        compiler = "bin/ldmd2",
        compiler_flags = [],
        data_globs = [],
        druntime = "lib/libdruntime-ldc.a",
        dub = "bin/dub",
        exec_compatible_with = ["@platforms//os:linux", "@platforms//cpu:x86_64"],
        ldc = "bin/ldc2",
        linkopts = ["-ldl", "-lm", "-lpthread", "-lrt"],
        phobos = "lib/libphobos2-ldc.a",
        runtime = ["gcc-12-base", "libgcc-s1", "zlib1g"],
        runtime_deps = [],
        sha256 = _cell("linux_x86_64")["sha256"],
        strip_prefix = _cell("linux_x86_64")["strip_prefix"],
        urls = _cell("linux_x86_64")["urls"],
        wrapper = True,
    ),
    "linux_arm64": struct(
        addon = None,
        archive = _cell("linux_arm64")["urls"][0].rpartition("/")[2],
        compiler = "bin/ldmd2",
        compiler_flags = [],
        data_globs = [],
        druntime = "lib/libdruntime-ldc.a",
        dub = "bin/dub",
        exec_compatible_with = ["@platforms//os:linux", "@platforms//cpu:aarch64"],
        ldc = "bin/ldc2",
        linkopts = ["-ldl", "-lm", "-lpthread", "-lrt"],
        phobos = "lib/libphobos2-ldc.a",
        runtime = ["gcc-12-base", "libgcc-s1", "zlib1g"],
        runtime_deps = [],
        sha256 = _cell("linux_arm64")["sha256"],
        strip_prefix = _cell("linux_arm64")["strip_prefix"],
        urls = _cell("linux_arm64")["urls"],
        wrapper = True,
    ),
    "macos_arm64": struct(
        addon = None,
        archive = _cell("macos_arm64")["urls"][0].rpartition("/")[2],
        compiler = "bin/ldmd2",
        compiler_flags = [],
        data_globs = [],
        druntime = "lib/libdruntime-ldc.a",
        dub = "bin/dub",
        exec_compatible_with = ["@platforms//os:macos", "@platforms//cpu:aarch64"],
        ldc = "bin/ldc2",
        linkopts = ["-ldl", "-lm", "-lpthread"],
        phobos = "lib/libphobos2-ldc.a",
        runtime = [],
        runtime_deps = [],
        sha256 = _cell("macos_arm64")["sha256"],
        strip_prefix = _cell("macos_arm64")["strip_prefix"],
        urls = _cell("macos_arm64")["urls"],
        wrapper = True,
    ),
    "windows_arm64": struct(
        addon = _WINDOWS_ADDON,
        archive = _cell("windows_arm64")["urls"][0].rpartition("/")[2],
        compiler = "bin/ldmd2.exe",
        compiler_flags = ["-mscrtlib=msvcrt", "-mtriple=aarch64-windows-msvc"],
        data_globs = ["bin/*.dll", "lib-windows-arm64/*.dll"],
        druntime = "lib-windows-arm64/druntime-ldc.lib",
        dub = "bin/dub.exe",
        exec_compatible_with = ["@platforms//os:windows", "@platforms//cpu:aarch64", MSVC_ABI],
        ldc = "bin/ldc2.exe",
        linkopts = _WINDOWS_LINKOPTS,
        phobos = "lib-windows-arm64/phobos2-ldc.lib",
        runtime = [],
        runtime_deps = ["//d/private/toolchain:msvc_iso_stdio"],
        sha256 = _cell("windows_arm64")["sha256"],
        strip_prefix = _cell("windows_arm64")["strip_prefix"],
        urls = _cell("windows_arm64")["urls"],
        wrapper = False,
    ),
    "windows_x86_64": struct(
        addon = None,
        archive = _cell("windows_x86_64")["urls"][0].rpartition("/")[2],
        compiler = "bin/ldmd2.exe",
        compiler_flags = ["-mscrtlib=msvcrt"],
        data_globs = ["bin/*.dll"],
        druntime = "lib/druntime-ldc.lib",
        dub = "bin/dub.exe",
        exec_compatible_with = ["@platforms//os:windows", "@platforms//cpu:x86_64", MSVC_ABI],
        ldc = "bin/ldc2.exe",
        linkopts = _WINDOWS_LINKOPTS,
        phobos = "lib/phobos2-ldc.lib",
        runtime = [],
        runtime_deps = ["//d/private/toolchain:msvc_iso_stdio"],
        sha256 = _cell("windows_x86_64")["sha256"],
        strip_prefix = _cell("windows_x86_64")["strip_prefix"],
        urls = _cell("windows_x86_64")["urls"],
        wrapper = False,
    ),
}

LDC_CROSS_CELLS = {
    "linux_arm64_to_linux_x86_64": struct(host = "linux_arm64", target = "linux_x86_64", triple = "x86_64-linux-gnu"),
    "linux_x86_64_to_linux_arm64": struct(host = "linux_x86_64", target = "linux_arm64", triple = "aarch64-linux-gnu"),
    "macos_arm64_to_linux_arm64": struct(host = "macos_arm64", target = "linux_arm64", triple = "aarch64-linux-gnu"),
    "windows_arm64_to_windows_x86_64": struct(host = "windows_arm64", target = "windows_x86_64", triple = "x86_64-windows-msvc"),
    "windows_x86_64_to_windows_arm64": struct(host = "windows_x86_64", target = "windows_arm64", triple = "aarch64-windows-msvc"),
}

LIBDPARSE_VERSION = _ARCHIVES["dx_d_libdparse"]["version"]

DCD_VERSION = _ARCHIVES["dx_d_dcd"]["version"]

CONTAINERS_VERSION = _ARCHIVES["dx_d_containers"]["version"]

INIFILED_VERSION = _ARCHIVES["dx_d_inifiled"]["version"]

LIBDDOC_VERSION = _ARCHIVES["dx_d_libddoc"]["version"]
