"""Haskell toolchain pin and settings tests."""

load("//libs/starlark:defs.bzl", "expect_contains", "expect_equal", "expect_false", "expect_true", "starlark_test")
load(":pins.bzl", "CELLS", "GHC_VERSION", "GLIBC_FLOOR", "RULES_HASKELL_COMMIT", "RULES_HASKELL_SHA256")
load(":repos.bzl", "CLOSURE_DIR", "NO_HOST_TOOL", "cell_matches_host", "closure_library_dirs", "ghc_library_dir", "ghc_settings")

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
library-dirs: ${pkgroot}/../lib/{triple}-ghc-9.10.3/ghc-bignum-1.3-0000
dynamic-library-dirs: ${pkgroot}/../lib/{triple}-ghc-9.10.3
extra-libraries: gmp"""

def _version_at_least(version, floor):
    return [int(part) for part in version.split(".")] >= [int(part) for part in floor.split(".")]

def _cell_checks(name):
    cell = CELLS[name]
    triple = cell.ghc_libdir_triple
    has_runtime = len(cell.runtime) > 0
    conf = closure_library_dirs(_CONF.replace("{triple}", triple), CLOSURE_DIR)
    arch_dir = {"linux_amd64": "x86_64-linux-gnu", "linux_arm64": "aarch64-linux-gnu", "macos_arm64": ""}[name]
    return [
        expect_equal(name + " ghc digest", len(cell.ghc_sha256), 64),
        expect_equal(name + " runtime packages", [package.package for package in cell.runtime], ["libgmp10", "libtinfo6"] if has_runtime else []),
        expect_true(name + " names a locale", len(cell.locale) > 0),
        expect_equal(name + " every package pins a digest", [package.package for package in cell.runtime if len(package.sha256) != 64], []),
        expect_equal(name + " every library ships a license", [package.package for package in cell.runtime if package.libraries and package.license == ""], []),
        expect_equal(name + " every library ships a copyright", [package.package for package in cell.runtime if package.libraries and package.copyright == ""], []),
        expect_equal(name + " libraries come from its own architecture", [path for package in cell.runtime for path in package.libraries.values() if arch_dir not in path], []),
        expect_equal(name + " debs come from its own architecture", [package.deb for package in cell.runtime if not package.deb.endswith("_" + cell.deb_arch + ".deb")], []),
        expect_equal(name + " library dir names the version", ghc_library_dir(name), "lib/" + triple + "-ghc-9.10.3"),
        expect_equal(name + " python is a repo label", cell.python.startswith("@python_3_12_"), True),
        expect_contains(name + " gmp resolves in the closure", conf, "library-dirs: ${pkgroot}/../dx_closure/lib ${pkgroot}/../lib/" + triple + "-ghc-9.10.3/ghc-bignum-1.3-0000"),
        expect_contains(name + " shared gmp resolves in the closure", conf, "dynamic-library-dirs: ${pkgroot}/../dx_closure/lib ${pkgroot}/../lib/" + triple + "-ghc-9.10.3"),
        expect_contains(name + " gmp stays an extra library", conf, "extra-libraries: gmp"),
    ]

def _checks():
    settings = ghc_settings(_SETTINGS)
    checks = [
        expect_equal("ghc version", GHC_VERSION, "9.10.3"),
        expect_equal("rules_haskell commit", RULES_HASKELL_COMMIT, "826018312ea91c80ff66fde1e1e39f76c42c07b8"),
        expect_equal("rules_haskell digest", len(RULES_HASKELL_SHA256), 64),
        expect_equal("cells", sorted(CELLS.keys()), ["linux_amd64", "linux_arm64", "macos_arm64"]),
        expect_true("glibc floor covers the bindist", _version_at_least(GLIBC_FLOOR, "2.28")),
        expect_equal("runtime package names match across cells", [[package.package for package in CELLS[name].runtime] for name in sorted(CELLS) if name.startswith("linux_")], [["libgmp10", "libtinfo6"]] * 2),
        expect_equal("darwin has no runtime closure", CELLS["macos_arm64"].runtime, []),
        expect_true("arm64 mac runs the macos cell", cell_matches_host("macos_arm64", "mac os x", "aarch64")),
        expect_false("linux never runs the macos cell", cell_matches_host("macos_arm64", "linux", "aarch64")),
        expect_true("amd64 host runs the amd64 cell", cell_matches_host("linux_amd64", "Linux", "amd64")),
        expect_true("x86_64 host runs the amd64 cell", cell_matches_host("linux_amd64", "linux", "x86_64")),
        expect_false("amd64 host never runs the arm64 cell", cell_matches_host("linux_arm64", "linux", "amd64")),
        expect_true("aarch64 host runs the arm64 cell", cell_matches_host("linux_arm64", "linux", "aarch64")),
        expect_true("arm64 host runs the arm64 cell", cell_matches_host("linux_arm64", "linux", "arm64")),
        expect_false("arm64 host never runs the amd64 cell", cell_matches_host("linux_amd64", "linux", "aarch64")),
        expect_false("other systems run no linux cell", cell_matches_host("linux_amd64", "mac os x", "x86_64")),
        expect_false("settings name no host path", "\"/" in settings),
        expect_contains("the C compiler comes from the cc toolchain", settings, "(\"C compiler command\", \"" + NO_HOST_TOOL + "\")"),
        expect_contains("merge objects comes from the cc toolchain", settings, "(\"Merge objects command\", \"" + NO_HOST_TOOL + "\")"),
        expect_contains("host link flags are dropped", settings, "(\"C compiler link flags\", \"\")"),
        expect_contains("bundled programs stay relative", settings, "(\"unlit command\", \"$topdir/../bin/unlit\")"),
    ]
    for name in sorted(CELLS.keys()):
        checks += _cell_checks(name)
    return checks

def haskell_metadata_tests(name):
    """Declares the Haskell toolchain metadata test."""
    starlark_test(
        name = name,
        mode = "load",
        checks = _checks(),
    )
