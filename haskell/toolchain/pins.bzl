"""Pinned GHC bindist, native closure and rules_haskell identities."""

GHC_VERSION = "9.10.3"

RULES_HASKELL_COMMIT = "826018312ea91c80ff66fde1e1e39f76c42c07b8"

RULES_HASKELL_SHA256 = "86d126db53fa06ee935d94ee109420d3a68733377091f7914f62abe36c1972a1"

RULES_HASKELL_URL = "https://github.com/tweag/rules_haskell/archive/" + RULES_HASKELL_COMMIT + ".tar.gz"

GLIBC_FLOOR = "2.28"

RUNTIME_MIRROR = "https://snapshot.debian.org/archive"

CELLS = {
    "linux_amd64": struct(
        exec_compatible_with = ["@platforms//os:linux", "@platforms//cpu:x86_64"],
        ghc_sha256 = "90f4e3082e1f43b984579f16b6349bd406dfdf88bda85733d7ccd54cb6a8eda8",
        ghc_strip_prefix = "ghc-" + GHC_VERSION + "-x86_64-unknown-linux",
        ghc_url = "https://downloads.haskell.org/~ghc/" + GHC_VERSION + "/ghc-" + GHC_VERSION + "-x86_64-deb10-linux.tar.xz",
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
}
