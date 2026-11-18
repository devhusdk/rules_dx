"""Fortran toolchain pin and acquisition tests."""

load("@rules_fortran//fortran/private:providers.bzl", "FortranToolchainInfo")
load("//fortran/private/lock:hub.bzl", fpm_build_file = "build_file")
load("//core:defs.bzl", "expect_contains", "expect_equal", "expect_true", "starlark_test")
load("//core:toolchain.bzl", "hub_checks", "pins_hub")
load("//fortran/private/toolchain:pins.bzl", "FLANG_RT_PREFIX", "FLANG_VERSION", "FORTRAN_CELLS")
load("//fortran/private/toolchain:repos.bzl", "FLANG_RT_TREES", "HUB_NAME", "flang_major", "flang_rt_build")
load("//fortran/private/toolchain:toolchain.bzl", "toolchain_fields")

def _checks():
    fields = toolchain_fields("flang", "ar", depset(), "rt")
    hub = pins_hub(HUB_NAME, FORTRAN_CELLS.keys(), "//fortran/private/toolchain:toolchain_type", "both", ["compiler_files", "flang"])
    return [
        expect_contains("fpm packages link their locked dependencies", fpm_build_file({"deps": ["stdlib"], "name": "app"}), '["//stdlib"]'),
        expect_contains("fpm packages glob one pattern per Fortran extension", fpm_build_file({"name": "app"}), '"src/**/*." + ext for ext in FORTRAN_EXTENSIONS'),
        expect_contains("hub aliases the exec flang", hub, 'name = "flang"'),
        expect_equal("pinned cells", sorted(FORTRAN_CELLS.keys()), ["linux_arm64", "linux_x86_64", "macos_arm64"]),
        expect_contains("flang-rt prefix names the flang version", FLANG_RT_PREFIX, FLANG_VERSION),
        expect_equal("flang-rt major matches flang", flang_major(FLANG_RT_PREFIX.split("-")[-1]), flang_major(FLANG_VERSION)),
        expect_true("flang-rt sources are extracted", "flang-rt" in FLANG_RT_TREES),
        expect_equal("flang-rt build uses the hub flang", flang_rt_build("$(execpath {flang}) {compiler_files}", "hub"), "$(execpath @hub//:flang) @hub//:compiler_files"),
        expect_equal("the compiler doubles as the unused linker", fields["linker"], "flang"),
        expect_equal("the runtime is the flang-rt CcInfo", fields["runtime_ccinfo"], "rt"),
        expect_equal("no prebuilt runtime libraries", fields["runtime_libraries"], []),
        expect_equal("toolchain fields match the upstream provider", sorted(fields.keys()), sorted(dir(FortranToolchainInfo(**fields)))),
    ] + hub_checks(hub, "//fortran/private/toolchain:toolchain_type", len(FORTRAN_CELLS))

def fortran_metadata_tests(name, size):
    """Declares the Fortran toolchain metadata test."""
    starlark_test(
        name = name,
        size = size,
        mode = "load",
        checks = _checks(),
    )
