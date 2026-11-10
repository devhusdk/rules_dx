"""Fortran toolchain pin and acquisition tests."""

load("@rules_fortran//fortran/private:providers.bzl", "FortranToolchainInfo")
load("//fortran/private:fpm_lock.bzl", "fpm_lock_packages")
load("//libs/starlark:defs.bzl", "expect_contains", "expect_equal", "expect_true", "starlark_test")
load(":pins.bzl", "FLANG_RT_PREFIX", "FLANG_VERSION", "FORTRAN_CELLS")
load(":repos.bzl", "DROPPED_PATHS", "FLANG_RT_TREES", "flang_major", "flang_rt_build", "fpm_build", "fpm_hub_build", "hub_build")
load(":toolchain.bzl", "toolchain_fields")

_FPM_LOCK = """# comment
[dependencies.stdlib]
version = "0.7.0"
url = "https://example.com/stdlib.tar.gz"
sha256 = "AAAA"
strip_prefix = "stdlib-0.7.0"

[dependencies.app]
version = "1.0.0"
url = "https://example.com/app.tar.gz"
sha256 = "bbbb"
dependencies = ["stdlib"]
"""

def _checks():
    fpm = fpm_lock_packages(_FPM_LOCK)
    linux = FORTRAN_CELLS["linux_x86_64"]
    fields = toolchain_fields("flang", "ar", depset(), "rt")
    root = hub_build()["BUILD.bazel"]
    return [
        expect_equal("fpm.lock tables become pins", [(p["name"], p["version"], p["sha256"], p["strip_prefix"], p["deps"]) for p in fpm], [("stdlib", "0.7.0", "aaaa", "stdlib-0.7.0", []), ("app", "1.0.0", "bbbb", "", ["stdlib"])]),
        expect_contains("fpm packages link their locked dependencies", fpm_build(fpm[1], "hub"), '["@hub_stdlib//:stdlib"]'),
        expect_contains("the fpm hub aliases every package", fpm_hub_build(fpm, "hub"), 'actual = "@hub_app//:app"'),
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
