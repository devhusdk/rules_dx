"""Haskell toolchain pin and settings tests."""

load("//libs/starlark:defs.bzl", "expect_contains", "expect_equal", "expect_false", "expect_true", "starlark_test")
load(":pins.bzl", "CELLS", "GHC_VERSION", "GLIBC_FLOOR", "RULES_HASKELL_COMMIT", "RULES_HASKELL_SHA256")
load(":repos.bzl", "CLOSURE_DIR", "GHC_LIBRARY_DIR", "NO_HOST_TOOL", "closure_library_dirs", "ghc_settings")

_SETTINGS = """[("C compiler command", "/usr/bin/gcc")
,("C compiler flags", "--target=x86_64-linux-gnu")
,("C++ compiler command", "/usr/bin/g++")
,("C++ compiler flags", "--target=x86_64-linux-gnu")
,("C compiler link flags", "--target=x86_64-linux-gnu -fuse-ld=gold")
,("CPP command", "/usr/bin/gcc")
,("Haskell CPP command", "/usr/bin/gcc")
,("JavaScript CPP command", "/usr/bin/gcc")
,("C-- CPP command", "/usr/bin/gcc")
,("Merge objects command", "/usr/bin/ld.gold")
,("ar command", "/usr/bin/ar")
,("ranlib command", "/usr/bin/ranlib")
,("otool command", "otool")
,("install_name_tool command", "install_name_tool")
,("windres command", "/bin/false")
,("LLVM llc command", "llc")
,("LLVM opt command", "opt")
,("LLVM llvm-as command", "clang")
,("unlit command", "$topdir/../bin/unlit")
]"""

_CONF = """name: ghc-bignum
library-dirs: ${pkgroot}/../lib/x86_64-linux-ghc-9.10.3/ghc-bignum-1.3-0000
dynamic-library-dirs: ${pkgroot}/../lib/x86_64-linux-ghc-9.10.3
extra-libraries: gmp"""

def _version_at_least(version, floor):
    return [int(part) for part in version.split(".")] >= [int(part) for part in floor.split(".")]

def _checks():
    cell = CELLS["linux_amd64"]
    settings = ghc_settings(_SETTINGS)
    conf = closure_library_dirs(_CONF, CLOSURE_DIR)
    return [
        expect_equal("ghc version", GHC_VERSION, "9.10.3"),
        expect_equal("rules_haskell commit", RULES_HASKELL_COMMIT, "826018312ea91c80ff66fde1e1e39f76c42c07b8"),
        expect_equal("rules_haskell digest", len(RULES_HASKELL_SHA256), 64),
        expect_equal("cells", sorted(CELLS.keys()), ["linux_amd64"]),
        expect_true("glibc floor covers the bindist", _version_at_least(GLIBC_FLOOR, "2.28")),
        expect_equal("ghc digest", len(cell.ghc_sha256), 64),
        expect_equal("runtime packages", [package.package for package in cell.runtime], ["libgmp10", "libtinfo6"]),
        expect_equal("every package pins a digest", [package.package for package in cell.runtime if len(package.sha256) != 64], []),
        expect_equal("every library ships a license", [package.package for package in cell.runtime if package.libraries and package.license == ""], []),
        expect_equal("every library ships a copyright", [package.package for package in cell.runtime if package.libraries and package.copyright == ""], []),
        expect_equal("library dir names the version", GHC_LIBRARY_DIR, "lib/x86_64-linux-ghc-9.10.3"),
        expect_false("settings name no host path", "\"/" in settings),
        expect_contains("the C compiler comes from the cc toolchain", settings, "(\"C compiler command\", \"" + NO_HOST_TOOL + "\")"),
        expect_contains("merge objects comes from the cc toolchain", settings, "(\"Merge objects command\", \"" + NO_HOST_TOOL + "\")"),
        expect_contains("host link flags are dropped", settings, "(\"C compiler link flags\", \"\")"),
        expect_contains("bundled programs stay relative", settings, "(\"unlit command\", \"$topdir/../bin/unlit\")"),
        expect_contains("gmp resolves in the closure", conf, "library-dirs: ${pkgroot}/../dx_closure/lib ${pkgroot}/../lib/x86_64-linux-ghc-9.10.3/ghc-bignum-1.3-0000"),
        expect_contains("shared gmp resolves in the closure", conf, "dynamic-library-dirs: ${pkgroot}/../dx_closure/lib ${pkgroot}/../lib/x86_64-linux-ghc-9.10.3"),
        expect_contains("gmp stays an extra library", conf, "extra-libraries: gmp"),
    ]

def haskell_metadata_tests(name):
    """Declares the Haskell toolchain metadata test."""
    starlark_test(
        name = name,
        mode = "load",
        checks = _checks(),
    )
