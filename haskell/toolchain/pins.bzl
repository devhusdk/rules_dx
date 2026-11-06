"""Pinned GHC bindist, native closure and rules_haskell identities."""

GHC_VERSION = "9.10.3"

CABAL_VERSION = "3.18.2.0"

_CABAL_URL = "https://downloads.haskell.org/~cabal/cabal-install-" + CABAL_VERSION + "/cabal-install-" + CABAL_VERSION + "-"

RULES_HASKELL_COMMIT = "826018312ea91c80ff66fde1e1e39f76c42c07b8"

RULES_HASKELL_SHA256 = "86d126db53fa06ee935d94ee109420d3a68733377091f7914f62abe36c1972a1"

RULES_HASKELL_URL = "https://github.com/tweag/rules_haskell/archive/" + RULES_HASKELL_COMMIT + ".tar.gz"

GLIBC_FLOOR = "2.28"

RUNTIME_MIRROR = "https://snapshot.debian.org/archive"

_GMP_LICENSE = struct(
    name = "gmp",
    sha256 = "e3a994d82e644b03a792a930f574002658412f62407f5fee083f2555c5f23118",
    url = "https://www.gnu.org/licenses/lgpl-3.0.txt",
)

CELLS = {
    "linux_amd64": struct(
        blocker = "",
        exe_suffix = "",
        os_libraries = ["unix"],
        ghc_libdir_triple = "x86_64-linux",
        host = struct(os = ["linux"], arch = ["amd64", "x86_64"]),
        python = "@python_3_12_x86_64_unknown_linux_gnu//:bin/python3",
        deb_arch = "amd64",
        locale = "C.UTF-8",
        exec_compatible_with = ["@platforms//os:linux", "@platforms//cpu:x86_64"],
        ghc_sha256 = "90f4e3082e1f43b984579f16b6349bd406dfdf88bda85733d7ccd54cb6a8eda8",
        ghc_strip_prefix = "ghc-" + GHC_VERSION + "-x86_64-unknown-linux",
        ghc_url = "https://downloads.haskell.org/~ghc/" + GHC_VERSION + "/ghc-" + GHC_VERSION + "-x86_64-deb10-linux.tar.xz",
        cabal_sha256 = "b4f4c83135da77cfc18ccd6056398ca714a2dad86d5f635621ffdda8f9afe587",
        cabal_url = _CABAL_URL + "x86_64-linux-unknown.tar.xz",
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
        ghc_sha256 = "99b85c1948e58e310f4e29cd2c8724a18c246c07e12637d8cbd591a064138236",
        ghc_strip_prefix = "ghc-" + GHC_VERSION + "-aarch64-unknown-linux",
        ghc_url = "https://downloads.haskell.org/~ghc/" + GHC_VERSION + "/ghc-" + GHC_VERSION + "-aarch64-deb10-linux.tar.xz",
        cabal_sha256 = "e4d940131ab0b78943a4b5ce64a9ef5acc452ea849df197787eaeecb1c3bf276",
        cabal_url = _CABAL_URL + "aarch64-linux-unknown.tar.xz",
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
        ghc_sha256 = "9f50ddd87be5cb994c719402778d6c7fdd341934fd4fbc0fcc3ecb40d49f860c",
        ghc_strip_prefix = "ghc-" + GHC_VERSION + "-aarch64-apple-darwin",
        ghc_url = "https://downloads.haskell.org/~ghc/" + GHC_VERSION + "/ghc-" + GHC_VERSION + "-aarch64-apple-darwin.tar.xz",
        cabal_sha256 = "d91f76cf9539d927205858c42d0c1fe3868dea89f54a5dd6151f9c3fdd56f21f",
        cabal_url = _CABAL_URL + "aarch64-apple-darwin.tar.xz",
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
        ghc_sha256 = "ad17bdbee5f195d50024da1447f458071d9d8a34f90d76907c873abaf95d893f",
        ghc_strip_prefix = "ghc-" + GHC_VERSION + "-x86_64-unknown-mingw32",
        ghc_url = "https://downloads.haskell.org/~ghc/" + GHC_VERSION + "/ghc-" + GHC_VERSION + "-x86_64-unknown-mingw32.tar.xz",
        cabal_sha256 = "7430982fd2aedef41539155d819e7e24349c664206ecf8c6e53c1c31acdbe630",
        cabal_url = _CABAL_URL + "x86_64-mingw64.zip",
        runtime = [],
        # GMP is linked statically into the Windows bindist.
        gmp_license = _GMP_LICENSE,
        mingw_cc = True,
    ),
}

def qualified_cells(cells = CELLS):
    """Returns the cells that are not blocked."""
    return [cell for cell in sorted(cells.keys()) if cells[cell].blocker == ""]
