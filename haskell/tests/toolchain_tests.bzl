"""Haskell toolchain pin and settings tests."""

load("//core:defs.bzl", "expect_contains", "expect_equal", "expect_false", "expect_true", "starlark_test")
load("//core:toolchain.bzl", "hub_checks")
load("//haskell/private/toolchain:packages.bzl", "cabal_libraries", "components", "is_sha256", "package_build", "package_deps", "pin_problems", "tool_labels")
load("//haskell/private/toolchain:pins.bzl", "CELLS", "GLIBC_FLOOR", "qualified_cells")
load("//haskell/private/toolchain:repos.bzl", "CLOSURE_DIR", "NO_HOST_TOOL", "cell_matches_host", "closure_library_dirs", "drop_host_sdk_dirs", "dropped_import_libs", "ghc_library_dir", "ghc_settings", "host_path", "hub_files", "os_libraries", "toolchain_libraries")

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

_DARWIN_RTS_CONF = """name: rts
include-dirs:
    ${pkgroot}/../lib/aarch64-osx-ghc-9.10.3/rts-1.0.2/include
    /Library/Developer/CommandLineTools/SDKs/MacOSX.sdk/usr/include/ffi
library-dirs:
    ${pkgroot}/../lib/aarch64-osx-ghc-9.10.3/rts-1.0.2
    /Library/Developer/CommandLineTools/SDKs/MacOSX.sdk/usr/lib
library-dirs-static:
    ${pkgroot}/../lib/aarch64-osx-ghc-9.10.3/rts-1.0.2
    /Library/Developer/CommandLineTools/SDKs/MacOSX.sdk/usr/lib
dynamic-library-dirs:
    ${pkgroot}/../lib/aarch64-osx-ghc-9.10.3
    /Library/Developer/CommandLineTools/SDKs/MacOSX.sdk/usr/lib
extra-libraries: m dl ffi"""

def _version_at_least(version, floor):
    return [int(part) for part in version.split(".")] >= [int(part) for part in floor.split(".")]

def _cell_checks(name):
    cell = CELLS[name]
    triple = cell.ghc_libdir_triple
    has_runtime = len(cell.runtime) > 0
    windows = name.startswith("windows_")
    conf = closure_library_dirs(_CONF.replace("{triple}", triple), CLOSURE_DIR)
    arch_dir = {"linux_x86_64": "x86_64-linux-gnu", "linux_arm64": "aarch64-linux-gnu", "macos_arm64": "", "windows_x86_64": ""}[name]
    return [
        expect_equal(name + " runtime packages", [package.package for package in cell.runtime], ["libgmp10", "libtinfo6"] if has_runtime else []),
        expect_true(name + " names a locale", len(cell.locale) > 0),
        expect_equal(name + " every library ships a license", [package.package for package in cell.runtime if package.libraries and package.license == ""], []),
        expect_equal(name + " every library ships a copyright", [package.package for package in cell.runtime if package.libraries and package.copyright == ""], []),
        expect_equal(name + " libraries come from its own architecture", [path for package in cell.runtime for path in package.libraries.values() if arch_dir not in path], []),
        expect_equal(name + " debs come from its own architecture", [package.deb for package in cell.runtime if not package.deb.endswith("_" + cell.deb_arch + ".deb")], []),
        expect_equal(name + " library dir names the version", ghc_library_dir(name), "lib/" + triple + "-ghc-9.10.3"),
        expect_equal(name + " python is a repo label", cell.python.startswith("@python_3_12_"), True),
        expect_equal(name + " python label ends in its interpreter", cell.python.endswith("//:python.exe" if windows else "//:bin/python3"), True),
        expect_equal(name + " executable suffix", cell.exe_suffix, ".exe" if windows else ""),
        expect_equal(name + " os libraries", list(cell.os_libraries), ["Win32"] if windows else ["unix"]),
        expect_true(name + " ships the common libraries", "base" in toolchain_libraries(name)),
        expect_true(name + " without a runtime closure ships the gmp license", has_runtime or cell.gmp_license.sha256 != ""),
        expect_contains(name + " gmp resolves in the closure", conf, "library-dirs: ${pkgroot}/../dx_closure/lib ${pkgroot}/../lib/" + triple + "-ghc-9.10.3/ghc-bignum-1.3-0000"),
        expect_contains(name + " shared gmp resolves in the closure", conf, "dynamic-library-dirs: ${pkgroot}/../dx_closure/lib ${pkgroot}/../lib/" + triple + "-ghc-9.10.3"),
        expect_contains(name + " gmp stays an extra library", conf, "extra-libraries: gmp"),
    ]

_SUBLIBRARY_CABAL = """cabal-version: 3.0
name: happy-lib
version: 2.1.7

common common-stanza
  build-depends: tabular

common modules
  exposed-modules: Happy.Grammar

library grammar
  import: common-stanza, modules
  build-depends:       base < 5, array

Library frontend
  build-depends:       base < 5, array, mtl, grammar
  exposed-modules:     Happy.Frontend,
                       Happy.Tabular

library tabular
  if impl(ghc >= 9)
    build-depends:
      base,
      -- grammar comes next
      happy-lib:grammar
  build-depends: frontend-extra

library
  build-depends: base >= 4.9 && < 5,
                 happy-lib:{frontend, tabular}

executable happy
  build-depends: base, grammar
"""

def _hub_names(hub):
    return [line.split("\"")[1] for line in hub.splitlines() if line.startswith("    name = ")]

def _checks():
    darwin_rts = drop_host_sdk_dirs(_DARWIN_RTS_CONF)
    settings = ghc_settings(_SETTINGS)
    windows_settings = ghc_settings(_SETTINGS.replace("\"Merge objects command\", \"/usr/bin/ld.gold\"", "\"Merge objects command\", \"\"").replace("\"ar command\", \"/usr/bin/ar\"", "\"ar command\", \"$topdir/../mingw//bin/llvm-ar.exe\"").replace("\"windres command\", \"/bin/false\"", "\"windres command\", \"$topdir/../mingw//bin/llvm-windres.exe\""))
    checks = hub_checks(hub_files()["BUILD.bazel"], str(Label("@rules_haskell//haskell:toolchain")), 5, ["@rules_haskell"]) + [
        expect_equal("cells", sorted(CELLS.keys()), ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_x86_64"]),
        expect_equal("qualified cells", qualified_cells(), ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_x86_64"]),
        expect_equal("windows is unblocked", CELLS["windows_x86_64"].blocker, ""),
        expect_true("windows host runs the windows cell", cell_matches_host("windows_x86_64", "windows server 2022", "amd64")),
        expect_false("linux never runs the windows cell", cell_matches_host("windows_x86_64", "linux", "amd64")),
        expect_equal("hub files", sorted(hub_files().keys()), ["BUILD.bazel", "lib/BUILD.bazel"]),
        expect_equal("hub toolchain order", _hub_names(hub_files()["BUILD.bazel"]), ["cc_windows_x86_64", "linux_arm64", "linux_x86_64", "macos_arm64", "windows_x86_64"]),
        expect_equal("windows cell gets one gated cc toolchain", hub_files()["BUILD.bazel"].count("@bazel_tools//tools/cpp:toolchain_type"), 1),
        expect_contains("windows cell cc toolchain is gated", hub_files()["BUILD.bazel"], "target_settings = [\"@@//haskell/private/toolchain:ghc_mingw_enabled\"]"),
        expect_true("glibc floor covers the bindist", _version_at_least(GLIBC_FLOOR, "2.28")),
        expect_equal("runtime package names match across cells", [[package.package for package in CELLS[name].runtime] for name in sorted(CELLS) if name.startswith("linux_")], [["libgmp10", "libtinfo6"]] * 2),
        expect_equal("darwin has no runtime closure", CELLS["macos_arm64"].runtime, []),
        expect_true("arm64 mac runs the macos cell", cell_matches_host("macos_arm64", "mac os x", "aarch64")),
        expect_false("linux never runs the macos cell", cell_matches_host("macos_arm64", "linux", "aarch64")),
        expect_true("amd64 host runs the amd64 cell", cell_matches_host("linux_x86_64", "Linux", "amd64")),
        expect_true("x86_64 host runs the amd64 cell", cell_matches_host("linux_x86_64", "linux", "x86_64")),
        expect_false("amd64 host never runs the arm64 cell", cell_matches_host("linux_arm64", "linux", "amd64")),
        expect_true("aarch64 host runs the arm64 cell", cell_matches_host("linux_arm64", "linux", "aarch64")),
        expect_true("arm64 host runs the arm64 cell", cell_matches_host("linux_arm64", "linux", "arm64")),
        expect_false("arm64 host never runs the amd64 cell", cell_matches_host("linux_x86_64", "linux", "aarch64")),
        expect_false("other systems run no linux cell", cell_matches_host("linux_x86_64", "mac os x", "x86_64")),
        expect_false("settings name no host path", host_path(settings)),
        expect_false("windows settings keep bundled mingw paths", host_path(windows_settings)),
        expect_contains("windows settings keep the bundled windres", windows_settings, "(\"windres command\", \"$topdir/../mingw//bin/llvm-windres.exe\")"),
        expect_contains("windows settings keep the bundled merge objects tool", windows_settings, "(\"Merge objects command\", \"\")"),
        expect_contains("windows settings keep the bundled ar", windows_settings, "(\"ar command\", \"$topdir/../mingw//bin/llvm-ar.exe\")"),
        expect_true("a drive path is a host path", host_path("(\"ar command\", \"C:\\\\tools\\\\ar.exe\")")),
        expect_true("an absolute path is a host path", host_path("(\"ar command\", \"/usr/bin/ar\")")),
        expect_false("a tooldir path is not a host path", host_path("(\"ar command\", \"$tooldir/ar\")")),
        expect_equal("nested import libraries are dropped", dropped_import_libs(["mingw/lib/libz.dll.a", "mingw/lib/libz.a", "mingw/x/y/libc++.dll.a"]), ["mingw/lib/libz.dll.a", "mingw/x/y/libc++.dll.a"]),
        expect_equal("os libraries", os_libraries(), ["Win32", "unix"]),
        expect_contains("Win32 is compatible on the windows cell", hub_files()["lib/BUILD.bazel"], "name = \"Win32\",\n    target_compatible_with = select({\n        \":windows_x86_64_platform\": [],\n        \"//conditions:default\": [\"@@platforms//:incompatible\"]"),
        expect_equal("each os library has an arm per qualified cell", hub_files()["lib/BUILD.bazel"].count("_platform\": [],"), 4),
        expect_contains("the C compiler comes from the cc toolchain", settings, "(\"C compiler command\", \"" + NO_HOST_TOOL + "\")"),
        expect_contains("merge objects comes from the cc toolchain", settings, "(\"Merge objects command\", \"" + NO_HOST_TOOL + "\")"),
        expect_contains("otool comes from the cc toolchain", settings, "(\"otool command\", \"" + NO_HOST_TOOL + "\")"),
        expect_contains("install_name_tool comes from the cc toolchain", settings, "(\"install_name_tool command\", \"" + NO_HOST_TOOL + "\")"),
        expect_contains("host link flags are dropped", settings, "(\"C compiler link flags\", \"\")"),
        expect_false("darwin rts conf drops the host sdk", "MacOSX.sdk" in darwin_rts),
        expect_equal("darwin rts conf keeps the bindist dirs", darwin_rts.count("${pkgroot}/../lib/aarch64-osx-ghc-9.10.3/rts-1.0.2"), 3),
        expect_contains("darwin rts conf keeps ffi", darwin_rts, "extra-libraries: m dl ffi"),
        expect_equal("linux conf is unchanged", drop_host_sdk_dirs(_CONF), _CONF),
        expect_contains("bundled programs stay relative", settings, "(\"unlit command\", \"$topdir/../bin/unlit\")"),
    ]
    archive = {"sha256": "a" * 64, "strip_prefix": "x-1.0", "urls": ["https://hackage.haskell.org/package/x-1.0/x-1.0.tar.gz"]}
    lock = {"schema": 1, "packages": [
        {"name": "alex", "version": "3.5", "archives": [archive], "deps": [], "extra": {"cabal_components": ["exe:alex"]}},
        {"name": "lib", "version": "1.0", "archives": [archive], "deps": ["alex"], "extra": {"cabal_components": ["lib"], "cabal_deps": ["alex", "base", "sub:inner"], "cabal_tools": ["alex:alex"]}},
        {"name": "sub", "version": "1.0", "archives": [archive], "deps": [], "extra": {"cabal_components": ["lib", "lib:inner"]}},
    ]}
    package = lock["packages"][1]
    build = package_build("lib", "1.0", components(package), package_deps("hub", lock, package), tool_labels("hub", package), {"": {"deps": [], "empty": False}})
    libraries = cabal_libraries(_SUBLIBRARY_CABAL)
    happy = package_build("happy-lib", "2.1.7", ["lib", "exe:happy"], ["@hub_base//:base"], [], libraries)
    checks += [
        expect_equal("cabal libraries name every library stanza with its sibling deps", {name: library["deps"] for name, library in libraries.items()}, {"grammar": [], "frontend": ["grammar"], "tabular": ["grammar"], "": ["frontend", "tabular"]}),
        expect_equal("libraries without modules are empty", sorted([name for name, library in libraries.items() if library["empty"]]), ["", "tabular"]),
        expect_contains("an empty library builds with empty cabal args", happy, 'haddock = False,\n    cabal_args = ":empty_library",'),
        expect_equal("a cabal file without sublibraries has only the main library", cabal_libraries("name: a\nlibrary\n  build-depends: base\n  exposed-modules: A\n"), {"": {"deps": [], "empty": False}}),
        expect_contains("a sublibrary builds as its own component", happy, 'name = "happy-lib-lib-frontend",\n    package_name = "happy-lib",\n    version = "2.1.7",\n    haddock = False,\n    sublibrary_name = "frontend",'),
        expect_contains("a sublibrary depends on its siblings", happy, 'deps = ["@hub_base//:base", ":happy-lib-lib-grammar"],'),
        expect_contains("the main library depends on its sublibraries", happy, 'deps = ["@hub_base//:base", ":happy-lib-lib-frontend", ":happy-lib-lib-tabular"],'),
        expect_contains("executables see every library of the package", happy, 'deps = ["@hub_base//:base", ":happy-lib", ":happy-lib-lib-frontend", ":happy-lib-lib-grammar", ":happy-lib-lib-tabular"],'),
        expect_false("sublibraries are no aliases", "alias(" in happy),
        expect_contains("package build renders tools", build, 'tools = ["@hub_alex//:alex-exe-alex"]'),
        expect_contains("a lock package dep points at its repository", build, '"@hub_alex//:alex"'),
        expect_contains("a dep outside the lock is a GHC boot package", build, '"@dx_haskell_toolchains//lib:base"'),
        expect_contains("a sublibrary dep points at its alias", build, '"@hub_sub//:sub-lib-inner"'),
        expect_false("lock deps alone do not name boot packages", "@dx_haskell_toolchains//lib:base" in package_deps("hub", lock, lock["packages"][1] | {"extra": {"cabal_components": ["lib"]}})),
    ]
    pinned = {"name": "pinned", "version": "1.0", "archives": [archive], "extra": {"cabal_revision": {"number": 2, "sha256": "b" * 64}}}
    checks += [
        expect_equal("pinned packages have no pin problem", pin_problems({"schema": 1, "packages": [pinned]}), []),
        expect_equal("an empty cabal digest of a revision names the package", pin_problems({"schema": 1, "packages": [pinned | {"name": "revised", "extra": {"cabal_revision": {"number": 1, "sha256": ""}}}]}), ['package revised has cabal revision sha256 "", expected 64 lowercase hex characters']),
        expect_equal("revision 0 needs no cabal digest", pin_problems({"schema": 1, "packages": [pinned | {"extra": {"cabal_revision": {"number": 0, "sha256": ""}}}]}), []),
        expect_equal("a package needs one sdist archive", pin_problems({"schema": 1, "packages": [pinned | {"name": "none", "archives": []}]}), ["package none has 0 archives, expected 1"]),
        expect_equal("a short sdist digest fails", len(pin_problems({"schema": 1, "packages": [pinned | {"archives": [archive | {"sha256": "a" * 63}]}]})), 1),
        expect_false("an uppercase digest is no sha256", is_sha256("A" * 64)),
        expect_false("a 65 character digest is no sha256", is_sha256("a" * 65)),
        expect_true("64 lowercase hex characters are a sha256", is_sha256("0123456789abcdef" * 4)),
    ]
    for name in sorted(CELLS.keys()):
        checks += _cell_checks(name)
    return checks

def haskell_metadata_tests(name, size):
    """Declares the Haskell toolchain metadata test."""
    starlark_test(
        name = name,
        size = size,
        mode = "load",
        checks = _checks(),
    )
