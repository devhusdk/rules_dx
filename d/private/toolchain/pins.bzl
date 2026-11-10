"""Pinned LDC compiler and rules_d identities; the archive data is in pins.json."""

load("@dx_pins_data//:pins.bzl", "DATA")

_PINS = DATA["dx_d"]

_ARCHIVES = _PINS["archives"]

RULES_D_VERSION = _ARCHIVES["rules_d"]["version"]

RULES_D_SHA256 = _ARCHIVES["rules_d"]["sha256"]

RULES_D_URLS = _ARCHIVES["rules_d"]["urls"]

LDC_VERSION = _PINS["version"]

# LDC ships no native Windows arm64 compiler: that cell runs the x64 compiler under emulation and
# takes its arm64 runtime libraries from the addon package. LDC's prebuilt libraries are MSVC-ABI, so
# both cells constrain on the msvc ABI of the hermetic cc toolchain.
_MSVC_ABI = "@llvm//constraints/windows/abi:msvc"

# druntime links legacy_stdio_definitions.lib; drop it and use //d/private/toolchain:msvc_iso_stdio instead (#6948).
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
        exec_compatible_with = ["@platforms//os:windows", "@platforms//cpu:aarch64", _MSVC_ABI],
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
        exec_compatible_with = ["@platforms//os:windows", "@platforms//cpu:x86_64", _MSVC_ABI],
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

LIBDPARSE_VERSION = _ARCHIVES["dx_d_libdparse"]["version"]

DCD_VERSION = _ARCHIVES["dx_d_dcd"]["version"]

CONTAINERS_VERSION = _ARCHIVES["dx_d_containers"]["version"]

INIFILED_VERSION = _ARCHIVES["dx_d_inifiled"]["version"]

LIBDDOC_VERSION = _ARCHIVES["dx_d_libddoc"]["version"]

TOOL_SOURCES = {
    "dx_d_containers": struct(
        dropped = ["doc-src", "test", "times.png", "dub.sdl", "meson.build"],
        files = {},
        sha256 = _ARCHIVES["dx_d_containers"]["sha256"],
        strip_prefix = _ARCHIVES["dx_d_containers"]["strip_prefix"],
        urls = _ARCHIVES["dx_d_containers"]["urls"],
    ),
    "dx_d_dcd": struct(
        # Only the BSL-1.0 dsymbol subpackage is kept; the GPL-3.0 client and server never reach a build.
        dropped = [
            "CHANGELOG.md",
            "README.md",
            "bash-completion",
            "build.bat",
            "build.sh",
            "buildloop.sh",
            "ci",
            "common",
            "constants-gen",
            "containers",
            "d-test-utils",
            "dub.json",
            "dub.selections.json",
            "installer",
            "libdparse",
            "makefile",
            "man1",
            "msgpack-d",
            "sonar-project.properties",
            "src",
            "teaser.png",
            "tests",
            "dsymbol/subprojects",
        ],
        files = {},
        sha256 = _ARCHIVES["dx_d_dcd"]["sha256"],
        strip_prefix = _ARCHIVES["dx_d_dcd"]["strip_prefix"],
        urls = _ARCHIVES["dx_d_dcd"]["urls"],
    ),
    "dx_d_inifiled": struct(
        dropped = ["test", "package.json"],
        files = {},
        sha256 = _ARCHIVES["dx_d_inifiled"]["sha256"],
        strip_prefix = _ARCHIVES["dx_d_inifiled"]["strip_prefix"],
        urls = _ARCHIVES["dx_d_inifiled"]["urls"],
    ),
    "dx_d_libddoc": struct(
        dropped = ["Makefile", "d-test-utils", "dub.json"],
        files = {},
        sha256 = _ARCHIVES["dx_d_libddoc"]["sha256"],
        strip_prefix = _ARCHIVES["dx_d_libddoc"]["strip_prefix"],
        urls = _ARCHIVES["dx_d_libddoc"]["urls"],
    ),
    "dx_d_libdparse": struct(
        dropped = ["dub.json"],
        files = {},
        sha256 = _ARCHIVES["dx_d_libdparse"]["sha256"],
        strip_prefix = _ARCHIVES["dx_d_libdparse"]["strip_prefix"],
        urls = _ARCHIVES["dx_d_libdparse"]["urls"],
    ),
}
