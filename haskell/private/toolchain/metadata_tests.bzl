"""Haskell toolchain pin and settings tests."""

load("//core:defs.bzl", "expect_contains", "expect_equal", "expect_false", "expect_true", "starlark_test")
load(":pins.bzl", "CELLS", "GLIBC_FLOOR", "qualified_cells")
load(":packages.bzl", "package_build")
load(":repos.bzl", "CLOSURE_DIR", "NO_HOST_TOOL", "cell_matches_host", "closure_library_dirs", "drop_host_sdk_dirs", "dropped_import_libs", "ghc_library_dir", "ghc_settings", "host_path", "hub_files", "os_libraries", "toolchain_libraries")

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

def _checks():
    darwin_rts = drop_host_sdk_dirs(_DARWIN_RTS_CONF)
    settings = ghc_settings(_SETTINGS)
    windows_settings = ghc_settings(_SETTINGS.replace("\"Merge objects command\", \"/usr/bin/ld.gold\"", "\"Merge objects command\", \"\"").replace("\"windres command\", \"/bin/false\"", "\"windres command\", \"$topdir/../mingw//bin/llvm-windres.exe\""))
    checks = [
        expect_false("toolchain hub loads nothing", "load(" in hub_files()["toolchains/BUILD.bazel"]),
        expect_false("toolchain hub names no rules_haskell", "@rules_haskell" in hub_files()["toolchains/BUILD.bazel"]),
        expect_equal("cells", sorted(CELLS.keys()), ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_x86_64"]),
        expect_equal("qualified cells", qualified_cells(), ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_x86_64"]),
        expect_equal("windows is unblocked", CELLS["windows_x86_64"].blocker, ""),
        expect_true("windows host runs the windows cell", cell_matches_host("windows_x86_64", "windows server 2022", "amd64")),
        expect_false("linux never runs the windows cell", cell_matches_host("windows_x86_64", "linux", "amd64")),
        expect_true("windows cell gets a hub target", "windows_x86_64" in hub_files()["toolchains/BUILD.bazel"]),
        expect_equal("windows cell gets one gated cc toolchain", hub_files()["cc/BUILD.bazel"].count("toolchain("), 1),
        expect_contains("windows cell cc toolchain is gated", hub_files()["cc/BUILD.bazel"], "target_settings = [\"@@//haskell/private/toolchain:ghc_mingw_enabled\"]"),
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
        expect_true("a drive path is a host path", host_path("(\"ar command\", \"C:\\\\tools\\\\ar.exe\")")),
        expect_true("an absolute path is a host path", host_path("(\"ar command\", \"/usr/bin/ar\")")),
        expect_false("a tooldir path is not a host path", host_path("(\"ar command\", \"$tooldir/ar\")")),
        expect_equal("nested import libraries are dropped", dropped_import_libs(["mingw/lib/libz.dll.a", "mingw/lib/libz.a", "mingw/x/y/libc++.dll.a"]), ["mingw/lib/libz.dll.a", "mingw/x/y/libc++.dll.a"]),
        expect_equal("os libraries", os_libraries(), ["Win32", "unix"]),
        expect_contains("Win32 is compatible on the windows cell", hub_files()["BUILD.bazel"], "name = \"Win32\",\n    target_compatible_with = select({\n        \":windows_x86_64_platform\": [],\n        \"//conditions:default\": [\"@@platforms//:incompatible\"]"),
        expect_equal("each os library has an arm per qualified cell", hub_files()["BUILD.bazel"].count("_platform\": [],"), 4),
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
    sidecar = {"packages": [
        {"name": "alex", "version": "3.5", "source": "hackage", "components": ["exe:alex"], "dependencies": [], "tools": []},
        {"name": "lib", "version": "1.0", "source": "hackage", "components": ["lib"], "dependencies": [], "tools": ["alex:alex"]},
    ]}
    build = package_build("hub", sidecar, sidecar["packages"][1])
    checks += [
        expect_contains("package build renders tools", build, 'tools = ["@hub_alex//:alex-exe-alex"]'),
        expect_false("package build keeps tools out of deps", 'deps = ["@hub_alex' in build),
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
