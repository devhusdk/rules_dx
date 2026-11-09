"""D toolchain pin and acquisition tests."""

load("//libs/starlark:defs.bzl", "expect_contains", "expect_equal", "expect_true", "starlark_test")
load(":pins.bzl", "CONTAINERS_VERSION", "DCD_VERSION", "INIFILED_VERSION", "LDC_CELLS", "LDC_RELEASES", "LDC_VERSION", "LIBDDOC_VERSION", "LIBDPARSE_VERSION", "RULES_D_SHA256", "RULES_D_URL", "RULES_D_VERSION", "TOOL_SOURCES")
load(":repos.bzl", "DROPPED_PATHS", "LDC_BUILD", "TOOL_BUILDS", "WRAPPER", "hub_build", "ldc_build", "ldc_required_paths", "ldc_runtime")

def _checks():
    linux = LDC_CELLS["linux_x86_64"]
    windows = [LDC_CELLS["windows_x86_64"], LDC_CELLS["windows_arm64"]]
    windows_build = ldc_build(LDC_CELLS["windows_x86_64"], "bin/ldmd2.exe")
    arm64_build = ldc_build(LDC_CELLS["windows_arm64"], "bin/ldmd2.exe")
    hub = hub_build()
    return [
        expect_true("hub loads nothing", "load(" not in hub),
        expect_true("hub never names rules_d", "@rules_d//" not in hub),
        expect_contains("hub uses the dx toolchain type", hub, "//d/toolchain/type:toolchain_type"),
        expect_equal("rules_d version", RULES_D_VERSION, "0.12.1"),
        expect_equal("rules_d digest", len(RULES_D_SHA256), 64),
        expect_contains("rules_d url names the version", RULES_D_URL, "v" + RULES_D_VERSION + ".tar.gz"),
        expect_equal("ldc version", LDC_VERSION, "1.43.0"),
        expect_equal("release url", LDC_RELEASES, "https://github.com/ldc-developers/ldc/releases/download/v1.43.0/"),
        expect_equal("pinned cells", sorted(LDC_CELLS.keys()), ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"]),
        expect_equal("arm64 linux archive", LDC_CELLS["linux_arm64"].archive, "ldc2-1.43.0-linux-aarch64.tar.xz"),
        expect_equal("macos archive", LDC_CELLS["macos_arm64"].archive, "ldc2-1.43.0-osx-arm64.tar.xz"),
        expect_true("every digest is a sha256", all([len(cell.sha256) == 64 for cell in LDC_CELLS.values()])),
        expect_true("every prefix names the version", all([LDC_VERSION in cell.strip_prefix for cell in LDC_CELLS.values()])),
        expect_equal("linux arm64 constraints", LDC_CELLS["linux_arm64"].exec_compatible_with, ["@platforms//os:linux", "@platforms//cpu:aarch64"]),
        expect_equal("macos constraints", LDC_CELLS["macos_arm64"].exec_compatible_with, ["@platforms//os:macos", "@platforms//cpu:aarch64"]),
        expect_true("macos has no librt", "-lrt" not in LDC_CELLS["macos_arm64"].linkopts),
        expect_equal("macos has no runtime closure", ldc_runtime("macos_arm64"), []),
        expect_equal("linux arm64 runtime closure", sorted([package.package for package in ldc_runtime("linux_arm64")]), ["gcc-12-base", "libgcc-s1", "zlib1g"]),
        expect_equal("linux archive", linux.archive, "ldc2-1.43.0-linux-x86_64.tar.xz"),
        expect_equal("linux digest", len(linux.sha256), 64),
        expect_contains("linux prefix names the version", linux.strip_prefix, LDC_VERSION),
        expect_equal("linux runtime closure", sorted([package.package for package in ldc_runtime("linux_x86_64")]), ["gcc-12-base", "libgcc-s1", "zlib1g"]),
        expect_true("the upstream toolchain extension is dropped", "d/extensions.bzl" in DROPPED_PATHS),
        expect_true("the upstream compiler download is dropped", "d/repositories.bzl" in DROPPED_PATHS),
        expect_true("the upstream module file is dropped", "MODULE.bazel" in DROPPED_PATHS),
        expect_true("acquisition requires the real compiler", "bin/ldc2" in ldc_required_paths(linux)),
        expect_equal("windows required paths", ldc_required_paths(LDC_CELLS["windows_x86_64"]), ["bin/ldc2.exe", "bin/ldmd2.exe", "etc/ldc2.conf", "lib/druntime-ldc.lib", "lib/phobos2-ldc.lib"]),
        expect_equal("windows arm64 required paths", ldc_required_paths(LDC_CELLS["windows_arm64"])[3:], ["lib-windows-arm64/druntime-ldc.lib", "lib-windows-arm64/phobos2-ldc.lib"]),
        expect_true("windows archives are 7z", all([cell.archive.endswith(".7z") for cell in windows])),
        expect_true("windows cells use the msvc abi", all([("@llvm//constraints/windows/abi:msvc" in cell.exec_compatible_with) for cell in windows])),
        expect_true("only windows arm64 has the addon", LDC_CELLS["windows_arm64"].addon != None and LDC_CELLS["windows_x86_64"].addon == None and linux.addon == None),
        expect_contains("windows arm64 targets arm64", LDC_CELLS["windows_arm64"].compiler_flags, "-mtriple=aarch64-windows-msvc"),
        expect_true("windows has no launcher", not any([cell.wrapper for cell in windows]) and linux.wrapper),
        expect_contains("windows names the exe compiler", windows_build, '"bin/ldmd2.exe"'),
        expect_true("windows never names the launcher", "dx_ldmd2" not in windows_build),
        expect_contains("windows links the msvc runtime", windows_build, 'static_library = "lib/druntime-ldc.lib"'),
        expect_contains("windows shims iso stdio", windows_build, 'deps = [":druntime_import"] + ["@@'),
        expect_equal("linux has no runtime deps", linux.runtime_deps, []),
        expect_contains("windows refuses legacy stdio", LDC_CELLS["windows_x86_64"].linkopts, "/NODEFAULTLIB:legacy_stdio_definitions.lib"),
        expect_contains("windows arm64 links the addon runtime", arm64_build, 'static_library = "lib-windows-arm64/phobos2-ldc.lib"'),
        expect_contains("linux ldmd2 sees the real compiler", ldc_build(linux, "bin/dx_ldmd2"), '"bin/ldc2"'),
        expect_contains("ldmd2 sees the compiler config", LDC_BUILD, '"etc/ldc2.conf/**"'),
        expect_contains("ldmd2 sees the runtime closure", LDC_BUILD, "{closure_glob}"),
        expect_true("the macos build has no closure glob", "/lib/**" not in ldc_build(LDC_CELLS["macos_arm64"], "w")),
        expect_contains("the wrapper loads the closure", WRAPPER, "/lib${{LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}}"),
        expect_equal("libdparse version", LIBDPARSE_VERSION, "0.25.1"),
        expect_equal("tool sources", sorted(TOOL_SOURCES.keys()), ["dx_d_containers", "dx_d_dcd", "dx_d_inifiled", "dx_d_libddoc", "dx_d_libdparse"]),
        expect_equal("libdparse digest", len(TOOL_SOURCES["dx_d_libdparse"].sha256), 64),
        expect_contains("libdparse url names the version", TOOL_SOURCES["dx_d_libdparse"].url, "v" + LIBDPARSE_VERSION + ".tar.gz"),
        expect_equal("dcd version", DCD_VERSION, "0.16.2"),
        expect_equal("containers version", CONTAINERS_VERSION, "0.9.0"),
        expect_equal("inifiled version", INIFILED_VERSION, "1.3.3"),
        expect_equal("libddoc version", LIBDDOC_VERSION, "0.8.0"),
        expect_true("every source digest is a sha256", all([len(source.sha256) == 64 for source in TOOL_SOURCES.values()])),
        expect_contains("dcd url names the version", TOOL_SOURCES["dx_d_dcd"].url, "v" + DCD_VERSION + ".tar.gz"),
        expect_contains("containers url names the version", TOOL_SOURCES["dx_d_containers"].url, "v" + CONTAINERS_VERSION + ".tar.gz"),
        expect_contains("inifiled url names the version", TOOL_SOURCES["dx_d_inifiled"].url, "v" + INIFILED_VERSION + ".tar.gz"),
        expect_contains("libddoc url names the version", TOOL_SOURCES["dx_d_libddoc"].url, "v" + LIBDDOC_VERSION + ".tar.gz"),
        expect_true("only dsymbol of the GPL-3.0 dcd is compiled", "src/dcd" not in TOOL_BUILDS["dx_d_dcd"] and "dsymbol/src/**" in TOOL_BUILDS["dx_d_dcd"]),
        expect_true("the dcd client and server sources are dropped", "src" in TOOL_SOURCES["dx_d_dcd"].dropped),
        expect_equal("one libdparse source", len([name for name in TOOL_SOURCES if "libdparse" in name]), 1),
        expect_contains("the wrapper runs ldmd2", WRAPPER, 'exec "$bin/ldmd2" "$@"'),
    ]

def d_metadata_tests(name):
    """Declares the D toolchain metadata test."""
    starlark_test(
        name = name,
        mode = "load",
        checks = _checks(),
    )
