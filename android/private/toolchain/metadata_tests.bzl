"""Android NDK toolchain selection coverage."""

load("//core:defs.bzl", "expect_contains", "expect_equal", "expect_true", "starlark_test")
load(":repos.bzl", "toolchain_build", "toolchain_repo_errors")

def _toolchain_checks():
    checks = []
    checks.append(expect_equal(
        "enabled selection is clean",
        toolchain_repo_errors("27.0.12077973", 31, "shared"),
        [],
    ))
    checks.append(expect_equal(
        "zero api is clean",
        toolchain_repo_errors("27.2.12479018", 0, "static"),
        [],
    ))
    checks.append(expect_equal(
        "other lineage fails",
        toolchain_repo_errors("26.3.11579264", 31, "shared"),
        ["android: ndk revision '26.3.11579264' is outside r27"],
    ))
    checks.append(expect_equal(
        "downgraded api fails",
        toolchain_repo_errors("27.0.12077973", 29, "shared"),
        ["android_ndk_toolchain: api_level 29 is below 31"],
    ))
    checks.append(expect_equal(
        "unknown runtime fails",
        toolchain_repo_errors("27.0.12077973", 31, "system"),
        ["android: cxx_runtime 'system' is unknown: want shared, static, none"],
    ))
    pinned = toolchain_build("@ndk_repo//", "external/ndk_repo/extracted", 31, "static")
    checks.append(expect_contains("pinned tools are labels", pinned, '"@ndk_repo//:clang", "@ndk_repo//:compiler_files"'))
    checks.append(expect_contains("device linker files", pinned, "@ndk_repo//:linker_files_aarch64-linux-android"))
    checks.append(expect_contains("emulator linker files", pinned, "@ndk_repo//:linker_files_x86_64-linux-android"))
    checks.append(expect_contains("clang version constant", pinned, 'clang_resource_dir = "external/ndk_repo/extracted/toolchains/llvm/prebuilt/linux-x86_64/lib/clang/18/include"'))
    checks.append(expect_contains("pinned clang is a label", pinned, 'clang = "@ndk_repo//:clang"'))
    checks.append(expect_true("pinned has no parent paths", "../" not in pinned))
    return checks

def android_toolchain_tests(name):
    """Declares the Android NDK toolchain selection pin test."""
    starlark_test(
        name = name,
        mode = "load",
        checks = _toolchain_checks(),
    )
