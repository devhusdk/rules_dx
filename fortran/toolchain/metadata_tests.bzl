"""Fortran toolchain pin and acquisition tests."""

load("@rules_fortran//fortran/private:providers.bzl", "FortranToolchainInfo")
load("//libs/starlark:defs.bzl", "expect_contains", "expect_equal", "expect_true", "starlark_test")
load(":pins.bzl", "FLANG_RT_PREFIX", "FLANG_RT_SHA256", "FLANG_RT_URL", "FLANG_VERSION", "FORTRAN_CELLS", "RULES_FORTRAN_COMMIT", "RULES_FORTRAN_SHA256", "RULES_FORTRAN_URL")
load(":repos.bzl", "DROPPED_PATHS", "FLANG_RT_TREES", "flang_major", "flang_rt_build")
load(":toolchain.bzl", "toolchain_fields")

def _checks():
    linux = FORTRAN_CELLS["linux_x86_64"]
    fields = toolchain_fields("flang", "ar", depset(), "rt")
    return [
        expect_equal("flang version", FLANG_VERSION, "21.1.8"),
        expect_equal("pinned cells", sorted(FORTRAN_CELLS.keys()), ["linux_arm64", "linux_x86_64", "macos_arm64"]),
        expect_equal("linux constraints", linux.constraints, ["@platforms//os:linux", "@platforms//cpu:x86_64"]),
        expect_contains("flang url names the version", linux.url, "/v21.1.8/flang+llvm-21.1.8-x86_64-unknown-linux-gnu.tar.gz"),
        expect_true("flang digests are sha256", all([len(c.sha256) == 64 for c in FORTRAN_CELLS.values()])),
        expect_contains("flang-rt url names the version", FLANG_RT_URL, "llvmorg-21.1.8/llvm-project-21.1.8.src.tar.xz"),
        expect_equal("flang-rt digest", len(FLANG_RT_SHA256), 64),
        expect_contains("flang-rt prefix names the flang version", FLANG_RT_PREFIX, FLANG_VERSION),
        expect_equal("flang-rt major matches flang", flang_major(FLANG_RT_PREFIX.split("-")[-1]), flang_major(FLANG_VERSION)),
        expect_equal("flang major", flang_major(FLANG_VERSION), "21"),
        expect_true("flang-rt sources are extracted", "flang-rt" in FLANG_RT_TREES),
        expect_equal("rules_fortran commit", len(RULES_FORTRAN_COMMIT), 40),
        expect_equal("rules_fortran digest", len(RULES_FORTRAN_SHA256), 64),
        expect_contains("rules_fortran url names the commit", RULES_FORTRAN_URL, RULES_FORTRAN_COMMIT),
        expect_true("the upstream module file is dropped", "MODULE.bazel" in DROPPED_PATHS),
        expect_equal("flang-rt build uses the hub flang", flang_rt_build("$(execpath {flang}) {compiler_files}", "hub"), "$(execpath @hub//:flang) @hub//:compiler_files"),
        expect_equal("the compiler doubles as the unused linker", fields["linker"], "flang"),
        expect_equal("the runtime is the flang-rt CcInfo", fields["runtime_ccinfo"], "rt"),
        expect_equal("no prebuilt runtime libraries", fields["runtime_libraries"], []),
        expect_equal("toolchain fields match the upstream provider", sorted(fields.keys()), sorted(dir(FortranToolchainInfo(**fields)))),
    ]

def fortran_metadata_tests(name):
    """Declares the Fortran toolchain metadata test."""
    starlark_test(
        name = name,
        mode = "load",
        checks = _checks(),
    )
