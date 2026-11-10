"""Pinned GHC bindist, native closure and rules_haskell identities; the archive data is in pins.json."""

load("@dx_pins_data//:pins.bzl", "DATA")

_PINS = DATA["dx_haskell"]

GHC_VERSION = _PINS["version"]

def _archive(name):
    pin = _PINS["archives"][name]
    return struct(sha256 = pin["sha256"], strip_prefix = pin.get("strip_prefix", ""), urls = pin["urls"])

RULES_HASKELL = _archive("rules_haskell")

_GMP_LICENSE = struct(
    name = "gmp",
    sha256 = _PINS["archives"]["gmp_license"]["sha256"],
    url = _PINS["archives"]["gmp_license"]["urls"][0],
)

GLIBC_FLOOR = "2.28"

RUNTIME_MIRROR = "https://snapshot.debian.org/archive"

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
        # GMP is linked into the Darwin bindist, so there is no runtime closure.
        runtime = [],
        # Ships the GMP LGPL text, which has no deb copyright file to come from.
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
        # GMP is linked statically into the Windows bindist.
        gmp_license = _GMP_LICENSE,
        mingw_cc = True,
    ),
}

def qualified_cells(cells = CELLS):
    """Returns the cells that are not blocked."""
    return [cell for cell in sorted(cells.keys()) if cells[cell].blocker == ""]
