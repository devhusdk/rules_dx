"""Pinned GHC bindist and native closure identities; the archive data is in pins.json."""

load("@dx_pins_data//:pins.bzl", "DATA")

_PINS = DATA["dx_haskell"]

GHC_VERSION = _PINS["version"]

def _archive(name):
    pin = _PINS["archives"][name]
    return struct(sha256 = pin["sha256"], strip_prefix = pin.get("strip_prefix", ""), urls = pin["urls"])

_GMP_LICENSE = struct(
    name = "gmp",
    sha256 = _PINS["archives"]["gmp_license"]["sha256"],
    url = _PINS["archives"]["gmp_license"]["urls"][0],
)

GLIBC_FLOOR = "2.28"

RUNTIME_MIRROR = "https://snapshot.debian.org/archive"

def runtime_urls(package):
    """Returns the snapshot.debian.org URL of one pinned runtime .deb."""
    return [RUNTIME_MIRROR + "/" + package.deb]

CELLS = {
    "linux_x86_64": struct(
        blocker = "",
        exe_suffix = "",
        os_libraries = ["unix"],
        ghc_libdir_triple = "x86_64-linux",
        host = struct(os = ["linux"], arch = ["amd64", "x86_64"]),
        python = "@python_3_12_x86_64_unknown_linux_gnu//:bin/python3",
        deb_arch = "amd64",
        locale = "C.UTF-8",
        exec_compatible_with = ["@platforms//os:linux", "@platforms//cpu:x86_64"],
        ghc = _archive("ghc__linux_x86_64"),
        cabal = _archive("cabal__linux_x86_64"),
        runtime = [
            struct(
                copyright = "usr/share/doc/libgmp10/copyright",
                deb = "debian/20240701T000000Z/pool/main/g/gmp/libgmp10_6.1.2+dfsg-4+deb10u1_amd64.deb",
                libraries = {"libgmp.so.10": "usr/lib/x86_64-linux-gnu/libgmp.so.10"},
                license = "LGPL-3.0-or-later OR GPL-2.0-or-later",
                package = "libgmp10",
                sha256 = "91f8037c4ffaf7937a957a33de939a04ad42d088c07b383c19051dfd8476036b",
            ),
            struct(
                copyright = "usr/share/doc/libtinfo6/copyright",
                deb = "debian-security/20240701T000000Z/pool/updates/main/n/ncurses/libtinfo6_6.1+20181013-2+deb10u5_amd64.deb",
                libraries = {"libtinfo.so.6": "lib/x86_64-linux-gnu/libtinfo.so.6"},
                license = "X11",
                package = "libtinfo6",
                sha256 = "cdb28b3ff0a280929bc012d15b7474c050ed48e2978a4653c1a83090c25ccdb0",
            ),
        ],
    ),
    "linux_arm64": struct(
        blocker = "",
        exe_suffix = "",
        os_libraries = ["unix"],
        ghc_libdir_triple = "aarch64-linux",
        host = struct(os = ["linux"], arch = ["aarch64", "arm64"]),
        python = "@python_3_12_aarch64_unknown_linux_gnu//:bin/python3",
        deb_arch = "arm64",
        locale = "C.UTF-8",
        exec_compatible_with = ["@platforms//os:linux", "@platforms//cpu:aarch64"],
        ghc = _archive("ghc__linux_arm64"),
        cabal = _archive("cabal__linux_arm64"),
        runtime = [
            struct(
                copyright = "usr/share/doc/libgmp10/copyright",
                deb = "debian/20240701T000000Z/pool/main/g/gmp/libgmp10_6.1.2+dfsg-4+deb10u1_arm64.deb",
                libraries = {"libgmp.so.10": "usr/lib/aarch64-linux-gnu/libgmp.so.10"},
                license = "LGPL-3.0-or-later OR GPL-2.0-or-later",
                package = "libgmp10",
                sha256 = "5ec3bd481106d1ce7d757f58f29dde5257056fdaa415b7ced01b7ff0bfafab0c",
            ),
            struct(
                copyright = "usr/share/doc/libtinfo6/copyright",
                deb = "debian-security/20240701T000000Z/pool/updates/main/n/ncurses/libtinfo6_6.1+20181013-2+deb10u5_arm64.deb",
                libraries = {"libtinfo.so.6": "lib/aarch64-linux-gnu/libtinfo.so.6"},
                license = "X11",
                package = "libtinfo6",
                sha256 = "1d5068958c16c86586e3f3acaf788a12bbdc12a0a0e07835d578be26c6fb4e9a",
            ),
        ],
    ),
    "macos_arm64": struct(
        blocker = "",
        exe_suffix = "",
        os_libraries = ["unix"],
        ghc_libdir_triple = "aarch64-osx",
        host = struct(os = ["mac"], arch = ["aarch64", "arm64"]),
        python = "@python_3_12_aarch64_apple_darwin//:bin/python3",
        deb_arch = "",
        locale = "en_US.UTF-8",
        exec_compatible_with = ["@platforms//os:macos", "@platforms//cpu:aarch64"],
        ghc = _archive("ghc__macos_arm64"),
        cabal = _archive("cabal__macos_arm64"),
        runtime = [],
        gmp_license = _GMP_LICENSE,
    ),
    "windows_x86_64": struct(
        blocker = "",
        exe_suffix = ".exe",
        os_libraries = ["Win32"],
        ghc_libdir_triple = "x86_64-windows",
        host = struct(os = ["windows"], arch = ["amd64", "x86_64"]),
        python = "@python_3_12_x86_64_pc_windows_msvc//:python.exe",
        deb_arch = "",
        locale = "C.UTF-8",
        exec_compatible_with = ["@platforms//os:windows", "@platforms//cpu:x86_64"],
        ghc = _archive("ghc__windows_x86_64"),
        cabal = _archive("cabal__windows_x86_64"),
        runtime = [],
        gmp_license = _GMP_LICENSE,
        mingw_cc = True,
    ),
}

def qualified_cells(cells = CELLS):
    """Returns the cells that are not blocked."""
    return [cell for cell in sorted(cells.keys()) if cells[cell].blocker == ""]

ALPINE_MIRROR = "https://dl-cdn.alpinelinux.org/alpine/v3.18/main"

MUSL_CELLS = {
    "linux_arm64_musl": struct(
        blocker = "",
        exe_suffix = "",
        os_libraries = ["unix"],
        ghc_libdir_triple = "aarch64-linux",
        host = struct(os = ["linux"], arch = ["aarch64", "arm64"]),
        python = "@python_3_12_aarch64_unknown_linux_gnu//:bin/python3",
        deb_arch = "",
        locale = "C.UTF-8",
        exec_compatible_with = ["@platforms//os:linux", "@platforms//cpu:aarch64"],
        target_compatible_with = ["@platforms//os:linux", "@platforms//cpu:aarch64", str(Label("@llvm//constraints/libc:musl"))],
        ghc = _archive("ghc__linux_arm64_musl"),
        cabal = _archive("cabal__linux_arm64"),
        runtime = [],
        loader = "ld-musl-aarch64.so.1",
        apks = [
            struct(package = "musl", apk = "aarch64/musl-1.2.4-r3.apk", sha256 = "d9674ebd60a55330fcca142a02e7e3e5a9ee889d65dc929311f53f735c5c8ac5", license = "MIT", libraries = {"ld-musl-aarch64.so.1": "lib/ld-musl-aarch64.so.1"}),
            struct(package = "libffi", apk = "aarch64/libffi-3.4.4-r2.apk", sha256 = "87d1e17785b654351efa2aca7ee27c61453237690802cdc90b21f1feaa323793", license = "MIT", libraries = {"libffi.so.8": "usr/lib/libffi.so.8.1.2"}),
            struct(package = "libncursesw", apk = "aarch64/libncursesw-6.4_p20230506-r0.apk", sha256 = "a7a3c246e0f3aa7e0666bb8ecb2718cc544db430a14fb7691c01b687cc4d43d2", license = "X11", libraries = {"libncursesw.so.6": "usr/lib/libncursesw.so.6.4"}),
            struct(package = "gmp", apk = "aarch64/gmp-6.2.1-r3.apk", sha256 = "13f042e07fe32b5487111d6dec4289f6e745cc950a6d64232be679f2435b715d", license = "LGPL-3.0-or-later OR GPL-2.0-or-later", libraries = {"libgmp.so.10": "usr/lib/libgmp.so.10.4.1"}),
            struct(package = "gmp-dev", apk = "aarch64/gmp-dev-6.2.1-r3.apk", sha256 = "c05f5c9a8fe1400607761a6946137ec338e99595645cab3e3038374e4e46e461", license = "LGPL-3.0-or-later OR GPL-2.0-or-later", libraries = {"libgmp.a": "usr/lib/libgmp.a"}),
            struct(package = "libstdc++", apk = "aarch64/libstdc++-12.2.1_git20220924-r10.apk", sha256 = "5d233534267802336584f8b203a83197d7ce4b1314497c65b22aae9a7935143b", license = "GPL-3.0-or-later WITH GCC-exception-3.1", libraries = {"libstdc++.so.6": "usr/lib/libstdc++.so.6.0.30"}),
            struct(package = "libgcc", apk = "aarch64/libgcc-12.2.1_git20220924-r10.apk", sha256 = "45907c59e25a6698019f9f39d3f4d61b7a5e11a7c41fcd3a7fa5a3a430e2a692", license = "GPL-3.0-or-later WITH GCC-exception-3.1", libraries = {"libgcc_s.so.1": "usr/lib/libgcc_s.so.1"}),
        ],
        loader_aliases = {
            "libc.musl-aarch64.so.1": "ld-musl-aarch64.so.1",
            "libc.so": "ld-musl-aarch64.so.1",
            "libdl.so": "ld-musl-aarch64.so.1",
            "libm.so": "ld-musl-aarch64.so.1",
            "libpthread.so": "ld-musl-aarch64.so.1",
            "librt.so": "ld-musl-aarch64.so.1",
            "libffi.so": "libffi.so.8",
            "libgmp.so": "libgmp.so.10",
            "libstdc++.so": "libstdc++.so.6",
        },
        gmp_license = _GMP_LICENSE,
    ),
}

ALL_CELLS = CELLS | MUSL_CELLS

# llvm_backend: the macOS GHC's AArch64 native code generator prints Mach-O GOT relocations
# whatever the target, so that cell compiles through llc and opt to emit ELF.
CROSS_CELLS = {
    "linux_arm64_to_linux_x86_64": struct(host = "linux_arm64", target = "linux_x86_64", cc_target = "x86_64-linux-gnu", llvm_backend = False),
    "linux_x86_64_to_linux_arm64": struct(host = "linux_x86_64", target = "linux_arm64", cc_target = "aarch64-linux-gnu", llvm_backend = False),
    "macos_arm64_to_linux_arm64": struct(host = "macos_arm64", target = "linux_arm64", cc_target = "aarch64-linux-gnu", llvm_backend = True),
}
