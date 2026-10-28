"""D toolchain pin and acquisition tests."""

load("//libs/starlark:defs.bzl", "expect_contains", "expect_equal", "expect_true", "starlark_test")
load(":pins.bzl", "CONTAINERS_VERSION", "DCD_VERSION", "DFMT_VERSION", "DSCANNER_VERSION", "INIFILED_VERSION", "LDC_CELLS", "LDC_RELEASES", "LDC_VERSION", "LIBDDOC_VERSION", "LIBDPARSE_VERSION", "RULES_D_SHA256", "RULES_D_URL", "RULES_D_VERSION", "TOOL_SOURCES")
load(":repos.bzl", "DROPPED_PATHS", "LDC_BUILD", "REQUIRED_PATHS", "TOOL_BUILDS", "WRAPPER", "ldc_runtime")

def _checks():
    linux = LDC_CELLS["linux_amd64"]
    return [
        expect_equal("rules_d version", RULES_D_VERSION, "0.12.1"),
        expect_equal("rules_d digest", len(RULES_D_SHA256), 64),
        expect_contains("rules_d url names the version", RULES_D_URL, "v" + RULES_D_VERSION + ".tar.gz"),
        expect_equal("ldc version", LDC_VERSION, "1.43.0"),
        expect_equal("release url", LDC_RELEASES, "https://github.com/ldc-developers/ldc/releases/download/v1.43.0/"),
        expect_equal("pinned cells", sorted(LDC_CELLS.keys()), ["linux_amd64", "linux_arm64", "macos_arm64"]),
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
        expect_equal("linux runtime closure", sorted([package.package for package in ldc_runtime("linux_amd64")]), ["gcc-12-base", "libgcc-s1", "zlib1g"]),
        expect_true("the upstream toolchain extension is dropped", "d/extensions.bzl" in DROPPED_PATHS),
        expect_true("the upstream compiler download is dropped", "d/repositories.bzl" in DROPPED_PATHS),
        expect_true("the upstream module file is dropped", "MODULE.bazel" in DROPPED_PATHS),
        expect_true("acquisition requires the real compiler", "bin/ldc2" in REQUIRED_PATHS),
        expect_contains("ldmd2 sees the real compiler", LDC_BUILD, '"bin/ldc2"'),
        expect_contains("ldmd2 sees the compiler config", LDC_BUILD, '"etc/ldc2.conf/**"'),
        expect_contains("ldmd2 sees the runtime closure", LDC_BUILD, "{closure_glob}"),
        expect_true("the macos build has no closure glob", "/lib/**" not in LDC_BUILD.format(closure = "c", closure_glob = "", linkopts = "[]", version_file = "v", wrapper = "w")),
        expect_contains("the wrapper loads the closure", WRAPPER, "/lib${{LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}}"),
        expect_equal("dfmt version", DFMT_VERSION, "0.15.2"),
        expect_equal("libdparse version", LIBDPARSE_VERSION, "0.25.1"),
        expect_equal("tool sources", sorted(TOOL_SOURCES.keys()), ["dx_d_containers", "dx_d_dcd", "dx_d_dfmt", "dx_d_dscanner", "dx_d_inifiled", "dx_d_libddoc", "dx_d_libdparse"]),
        expect_equal("dfmt digest", len(TOOL_SOURCES["dx_d_dfmt"].sha256), 64),
        expect_equal("libdparse digest", len(TOOL_SOURCES["dx_d_libdparse"].sha256), 64),
        expect_contains("dfmt url names the version", TOOL_SOURCES["dx_d_dfmt"].url, "v" + DFMT_VERSION + ".tar.gz"),
        expect_contains("libdparse url names the version", TOOL_SOURCES["dx_d_libdparse"].url, "v" + LIBDPARSE_VERSION + ".tar.gz"),
        expect_contains("dfmt builds with upstream rules_d", TOOL_BUILDS["dx_d_dfmt"], "@rules_d//d:defs.bzl"),
        expect_contains("dfmt is restricted to qualified cells", TOOL_BUILDS["dx_d_dfmt"], "D_COMPATIBLE_WITH"),
        expect_true("dfmt never builds through the dx wrapper", "rules_dx//d/rules:defs.bzl\", \"d_" not in TOOL_BUILDS["dx_d_dfmt"]),
        expect_contains("dfmt depends on libdparse", TOOL_BUILDS["dx_d_dfmt"], "@dx_d_libdparse//:dparse"),
        expect_equal("dscanner version", DSCANNER_VERSION, "0.16.0-beta.5"),
        expect_equal("dcd version", DCD_VERSION, "0.16.2"),
        expect_equal("containers version", CONTAINERS_VERSION, "0.9.0"),
        expect_equal("inifiled version", INIFILED_VERSION, "1.3.3"),
        expect_equal("libddoc version", LIBDDOC_VERSION, "0.8.0"),
        expect_true("every source digest is a sha256", all([len(source.sha256) == 64 for source in TOOL_SOURCES.values()])),
        expect_contains("dscanner url names the version", TOOL_SOURCES["dx_d_dscanner"].url, "v" + DSCANNER_VERSION + ".tar.gz"),
        expect_contains("dcd url names the version", TOOL_SOURCES["dx_d_dcd"].url, "v" + DCD_VERSION + ".tar.gz"),
        expect_contains("containers url names the version", TOOL_SOURCES["dx_d_containers"].url, "v" + CONTAINERS_VERSION + ".tar.gz"),
        expect_contains("inifiled url names the version", TOOL_SOURCES["dx_d_inifiled"].url, "v" + INIFILED_VERSION + ".tar.gz"),
        expect_contains("libddoc url names the version", TOOL_SOURCES["dx_d_libddoc"].url, "v" + LIBDDOC_VERSION + ".tar.gz"),
        expect_equal("dfmt githash", TOOL_SOURCES["dx_d_dfmt"].files, {"githash.txt": "v0.15.2\n"}),
        expect_equal("dscanner githash", TOOL_SOURCES["dx_d_dscanner"].files, {"githash.txt": "v0.16.0-beta.5\n"}),
        expect_contains("dscanner builds with upstream rules_d", TOOL_BUILDS["dx_d_dscanner"], "@rules_d//d:defs.bzl"),
        expect_contains("dscanner is restricted to qualified cells", TOOL_BUILDS["dx_d_dscanner"], "D_COMPATIBLE_WITH"),
        expect_contains("dscanner depends on the shared libdparse", TOOL_BUILDS["dx_d_dscanner"], "@dx_d_libdparse//:dparse"),
        expect_true("dscanner never builds through the dx wrapper", "rules_dx//d/rules:defs.bzl\", \"d_" not in TOOL_BUILDS["dx_d_dscanner"]),
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
