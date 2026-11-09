"""Fortran toolchain pin and acquisition tests."""

load("@rules_fortran//fortran/private:providers.bzl", "FortranToolchainInfo")
load("//libs/starlark:defs.bzl", "expect_contains", "expect_equal", "expect_true", "starlark_test")
load(":pins.bzl", "FLANG_RT_PREFIX", "FLANG_VERSION", "FORTRAN_CELLS")
load(":repos.bzl", "DROPPED_PATHS", "FLANG_RT_TREES", "flang_major", "flang_rt_build", "hub_build")
load(":toolchain.bzl", "toolchain_fields")

def _checks():
    linux = FORTRAN_CELLS["linux_x86_64"]
    fields = toolchain_fields("flang", "ar", depset(), "rt")
    root = hub_build()["BUILD.bazel"]
    return [
        expect_true("hub root loads nothing", "load(" not in root),
        expect_true("hub root names no rules_fortran", "@rules_fortran" not in root),
        expect_contains("hub root uses the rules_fortran type", root, "//fortran:toolchain_type"),
        expect_contains("hub impl holds the loads", hub_build()["impl/BUILD.bazel"], "load("),
        expect_equal("pinned cells", sorted(FORTRAN_CELLS.keys()), ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_x86_64"]),
        expect_contains("flang-rt prefix names the flang version", FLANG_RT_PREFIX, FLANG_VERSION),
        expect_equal("flang-rt major matches flang", flang_major(FLANG_RT_PREFIX.split("-")[-1]), flang_major(FLANG_VERSION)),
        expect_true("flang-rt sources are extracted", "flang-rt" in FLANG_RT_TREES),
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
