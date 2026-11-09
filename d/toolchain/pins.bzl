"""Pinned LDC compiler and rules_d identities."""

RULES_D_VERSION = "0.12.1"

RULES_D_SHA256 = "87aaf9bc99f0635df22efcbea67a95bce2d8b4fad172012e0314e2cb79149816"

RULES_D_URL = "https://github.com/bazel-contrib/rules_d/archive/refs/tags/v" + RULES_D_VERSION + ".tar.gz"

LDC_VERSION = "1.43.0"

LDC_RELEASES = "https://github.com/ldc-developers/ldc/releases/download/v" + LDC_VERSION + "/"

# LDC ships no native Windows arm64 compiler: that cell runs the x64 compiler under emulation and
# takes its arm64 runtime libraries from the addon package. LDC's prebuilt libraries are MSVC-ABI, so
# both cells constrain on the msvc ABI of the hermetic cc toolchain.
_MSVC_ABI = "@llvm//constraints/windows/abi:msvc"

_WINDOWS_ARCHIVE = "ldc2-" + LDC_VERSION + "-windows-x64.7z"

_WINDOWS_SHA256 = "60ae3d5e34287aa25433c7550520060a52f9f212e3d5ff6472a9067d8d17e47e"

# druntime links legacy_stdio_definitions.lib; drop it and use //d/toolchain:msvc_iso_stdio instead (#6948).
_WINDOWS_LINKOPTS = ["/NODEFAULTLIB:legacy_stdio_definitions.lib"]

_WINDOWS_ADDON = struct(
    archive = "ldc2-" + LDC_VERSION + "-addon-windows.tar.xz",
    keep = ["etc/ldc2.conf/55-target-windows-arm64.conf", "lib-windows-arm64"],
    sha256 = "88c6679eccfcbbf82b90dc15b731d07eca74aea088fe0a20e1a2780e4c2bc45d",
)

LDC_CELLS = {
    "linux_x86_64": struct(
        addon = None,
        archive = "ldc2-" + LDC_VERSION + "-linux-x86_64.tar.xz",
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
        sha256 = "35805db60f47fd1a162b2035da84d96beec37e6d1e7b1edf60498a90acfaedce",
        strip_prefix = "ldc2-" + LDC_VERSION + "-linux-x86_64",
        wrapper = True,
    ),
    "linux_arm64": struct(
        addon = None,
        archive = "ldc2-" + LDC_VERSION + "-linux-aarch64.tar.xz",
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
        sha256 = "f2b24edc254d4b9944234f641de3c4b3a36d5752639ec30cb2bbe122931b2283",
        strip_prefix = "ldc2-" + LDC_VERSION + "-linux-aarch64",
        wrapper = True,
    ),
    "macos_arm64": struct(
        addon = None,
        archive = "ldc2-" + LDC_VERSION + "-osx-arm64.tar.xz",
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
        sha256 = "5e332349c784d4e0b1c4622284803880f89d97ef1ef97c6b25aa4d6b094c5fed",
        strip_prefix = "ldc2-" + LDC_VERSION + "-osx-arm64",
        wrapper = True,
    ),
    "windows_arm64": struct(
        addon = _WINDOWS_ADDON,
        archive = _WINDOWS_ARCHIVE,
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
        runtime_deps = ["//d/toolchain:msvc_iso_stdio"],
        sha256 = _WINDOWS_SHA256,
        strip_prefix = "ldc2-" + LDC_VERSION + "-windows-x64",
        wrapper = False,
    ),
    "windows_x86_64": struct(
        addon = None,
        archive = _WINDOWS_ARCHIVE,
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
        runtime_deps = ["//d/toolchain:msvc_iso_stdio"],
        sha256 = _WINDOWS_SHA256,
        strip_prefix = "ldc2-" + LDC_VERSION + "-windows-x64",
        wrapper = False,
    ),
}

LIBDPARSE_VERSION = "0.25.1"

DCD_VERSION = "0.16.2"

CONTAINERS_VERSION = "0.9.0"

INIFILED_VERSION = "1.3.3"

LIBDDOC_VERSION = "0.8.0"

TOOL_SOURCES = {
    "dx_d_containers": struct(
        dropped = ["doc-src", "test", "times.png", "dub.sdl", "meson.build"],
        files = {},
        sha256 = "5e256b84bbdbd2bd625cba0472ea27a1fde6d673d37a85fe971a20d52874acaa",
        strip_prefix = "containers-" + CONTAINERS_VERSION,
        url = "https://github.com/dlang-community/containers/archive/refs/tags/v" + CONTAINERS_VERSION + ".tar.gz",
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
        sha256 = "3c333269fdd8b3b2f7278d4f3a5693ec8450547c2091312a805b9605149b205a",
        strip_prefix = "DCD-" + DCD_VERSION,
        url = "https://github.com/dlang-community/DCD/archive/refs/tags/v" + DCD_VERSION + ".tar.gz",
    ),
    "dx_d_inifiled": struct(
        dropped = ["test", "package.json"],
        files = {},
        sha256 = "017432594651ab12abaf93aae0918c3d609d0a65800d79168e0b4bd0326ca136",
        strip_prefix = "inifiled-" + INIFILED_VERSION,
        url = "https://github.com/burner/inifiled/archive/refs/tags/v" + INIFILED_VERSION + ".tar.gz",
    ),
    "dx_d_libddoc": struct(
        dropped = ["Makefile", "d-test-utils", "dub.json"],
        files = {},
        sha256 = "9e5657de8aea225f622a00b5cb5ca9f1dac091dd98970152d65876a92669e18e",
        strip_prefix = "libddoc-" + LIBDDOC_VERSION,
        url = "https://github.com/dlang-community/libddoc/archive/refs/tags/v" + LIBDDOC_VERSION + ".tar.gz",
    ),
    "dx_d_libdparse": struct(
        dropped = ["dub.json"],
        files = {},
        sha256 = "83825fda4ce67cac49e1bfdce0051a9e2499a5fadbf3a7f4d0853b487eaebc82",
        strip_prefix = "libdparse-" + LIBDPARSE_VERSION,
        url = "https://github.com/dlang-community/libdparse/archive/refs/tags/v" + LIBDPARSE_VERSION + ".tar.gz",
    ),
}
