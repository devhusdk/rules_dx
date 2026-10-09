"""Android NDK toolchain selection coverage."""

load("//libs/starlark:defs.bzl", "expect_contains", "expect_equal", "expect_true", "starlark_test")
load(":repos.bzl", "required_files", "toolchain_repo_errors")

_SDK = "/opt/android-sdk"

def _toolchain_checks():
    checks = []
    checks.append(expect_equal(
        "enabled selection is clean",
        toolchain_repo_errors(_SDK, "27.0.12077973", 31, "shared"),
        [],
    ))
    checks.append(expect_equal(
        "zero api is clean",
        toolchain_repo_errors(_SDK, "27.2.12479018", 0, "static"),
        [],
    ))
    checks.append(expect_equal(
        "missing path fails",
        toolchain_repo_errors("", "27.0.12077973", 31, "shared"),
        ["android_ndk_toolchain: local_path names the installed Android SDK"],
    ))
    checks.append(expect_equal(
        "other lineage fails",
        toolchain_repo_errors(_SDK, "26.3.11579264", 31, "shared"),
        ["android: ndk revision '26.3.11579264' is outside r27"],
    ))
    checks.append(expect_equal(
        "downgraded api fails",
        toolchain_repo_errors(_SDK, "27.0.12077973", 29, "shared"),
        ["android_ndk_toolchain: api_level 29 is below 31"],
    ))
    checks.append(expect_equal(
        "unknown runtime fails",
        toolchain_repo_errors(_SDK, "27.0.12077973", 31, "system"),
        ["android: cxx_runtime 'system' is unknown: want shared, static, none"],
    ))
    required = required_files(_SDK, "27.0.12077973")
    checks.append(expect_true("anchor is required", (_SDK + "/ndk/27.0.12077973/source.properties") in required))
    checks.append(expect_contains("clang is required", str(required), "toolchains/llvm/prebuilt/linux-x86_64/bin/clang"))
    checks.append(expect_contains("device sysroot is required", str(required), "sysroot/usr/lib/aarch64-linux-android"))
    checks.append(expect_contains("emulator sysroot is required", str(required), "sysroot/usr/lib/x86_64-linux-android"))
    return checks

def android_toolchain_tests(name):
    """Declares the Android NDK toolchain selection pin test."""
    starlark_test(
        name = name,
        mode = "load",
        checks = _toolchain_checks(),
    )
