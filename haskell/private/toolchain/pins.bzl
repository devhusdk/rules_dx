"""Pinned GHC bindist and native closure identities; the archive data is in pins.json."""

load("@dx_pins_data//:pins.bzl", "DATA")

_PINS = DATA["dx_haskell"]

GHC_VERSION = _PINS["version"]

def _archive(name):
    pin = _PINS["archives"][name]
    return struct(sha256 = pin["sha256"], strip_prefix = pin.get("strip_prefix", ""), urls = pin["urls"])

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
        gmp_license = True,
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
        gmp_license = True,
        mingw_cc = True,
    ),
}

def qualified_cells(cells = CELLS):
    """Returns the cells that are not blocked."""
    return [cell for cell in sorted(cells.keys()) if cells[cell].blocker == ""]

def _apk(package, cell, license, libraries):
    pin = _PINS["archives"]["apk__%s__%s" % (package, cell)]
    return struct(package = package, url = pin["urls"][0], sha256 = pin["sha256"], license = license, libraries = libraries)

def _musl_cell(cell, glibc, arch):
    loader = "ld-musl-%s.so.1" % arch
    return struct(
        blocker = "",
        exe_suffix = "",
        os_libraries = ["unix"],
        ghc_libdir_triple = glibc.ghc_libdir_triple,
        host = glibc.host,
        python = glibc.python,
        deb_arch = "",
        locale = "C.UTF-8",
        exec_compatible_with = glibc.exec_compatible_with,
        target_compatible_with = glibc.exec_compatible_with + [str(Label("@llvm//constraints/libc:musl"))],
        ghc = _archive("ghc__" + cell),
        cabal = glibc.cabal,
        runtime = [],
        loader = loader,
        apks = [
            _apk("musl", cell, license = "MIT", libraries = {loader: "lib/" + loader}),
            _apk("libffi", cell, license = "MIT", libraries = {"libffi.so.8": "usr/lib/libffi.so.8.1.2"}),
            _apk("libncursesw", cell, license = "X11", libraries = {"libncursesw.so.6": "usr/lib/libncursesw.so.6.4"}),
            _apk("gmp", cell, license = "LGPL-3.0-or-later OR GPL-2.0-or-later", libraries = {"libgmp.so.10": "usr/lib/libgmp.so.10.4.1"}),
            _apk("gmp-dev", cell, license = "LGPL-3.0-or-later OR GPL-2.0-or-later", libraries = {"libgmp.a": "usr/lib/libgmp.a"}),
            _apk("libstdc++", cell, license = "GPL-3.0-or-later WITH GCC-exception-3.1", libraries = {"libstdc++.so.6": "usr/lib/libstdc++.so.6.0.30"}),
            _apk("libgcc", cell, license = "GPL-3.0-or-later WITH GCC-exception-3.1", libraries = {"libgcc_s.so.1": "usr/lib/libgcc_s.so.1"}),
        ],
        loader_aliases = {
            "libc.musl-%s.so.1" % arch: loader,
            "libc.so": loader,
            "libdl.so": loader,
            "libm.so": loader,
            "libpthread.so": loader,
            "librt.so": loader,
            "libffi.so": "libffi.so.8",
            "libgmp.so": "libgmp.so.10",
            "libstdc++.so": "libstdc++.so.6",
        },
        gmp_license = True,
    )

MUSL_CELLS = {
    "linux_arm64_musl": _musl_cell("linux_arm64_musl", CELLS["linux_arm64"], "aarch64"),
    "linux_x86_64_musl": _musl_cell("linux_x86_64_musl", CELLS["linux_x86_64"], "x86_64"),
}

ALL_CELLS = CELLS | MUSL_CELLS

# llvm_backend: the macOS GHC's AArch64 native code generator prints Mach-O GOT relocations
# whatever the target, so that cell compiles through llc and opt to emit ELF.
CROSS_CELLS = {
    "linux_arm64_to_linux_x86_64": struct(host = "linux_arm64", target = "linux_x86_64", cc_target = "x86_64-linux-gnu", llvm_backend = False),
    "linux_x86_64_to_linux_arm64": struct(host = "linux_x86_64", target = "linux_arm64", cc_target = "aarch64-linux-gnu", llvm_backend = False),
    "macos_arm64_to_linux_arm64": struct(host = "macos_arm64", target = "linux_arm64", cc_target = "aarch64-linux-gnu", llvm_backend = True),
}
