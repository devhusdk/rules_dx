"""Android native tuple and configuration coverage."""

load("//libs/starlark:defs.bzl", "expect_contains", "expect_equal", "expect_false", "expect_true", "starlark_test")
load(
    ":defs.bzl",
    "api_floor",
    "clang_target",
    "constraint_list",
    "cpu_for",
    "cxx_runtime_errors",
    "default_library_config",
    "effective_api_level",
    "library_config_errors",
    "ndk_prebuilt_dir",
    "ndk_revision_errors",
    "platform_label",
    "rust_triples",
    "sysroot_api_dir",
    "triple_for",
    "tuple_errors",
    "tuples",
)

def _config(**kwargs):
    base = default_library_config("device")
    fields = ["api_level", "copts", "cxx_runtime", "features", "rustc_flags", "tuple"]
    return struct(**{key: kwargs.get(key, getattr(base, key)) for key in fields})

def _platform_checks():
    checks = []
    checks.append(expect_equal("tuples", tuples(), ["device", "emulator"]))
    checks.append(expect_equal("device triple", triple_for("device"), "aarch64-linux-android"))
    checks.append(expect_equal("emulator triple", triple_for("emulator"), "x86_64-linux-android"))
    checks.append(expect_equal("unknown triple is empty", triple_for("watch"), ""))
    checks.append(expect_equal("device cpu", cpu_for("device"), "@platforms//cpu:arm64"))
    checks.append(expect_equal("emulator cpu", cpu_for("emulator"), "@platforms//cpu:x86_64"))
    checks.append(expect_equal(
        "device constraints",
        constraint_list("device"),
        ["@platforms//os:android", "@platforms//cpu:arm64"],
    ))
    checks.append(expect_equal(
        "emulator constraints",
        constraint_list("emulator"),
        ["@platforms//os:android", "@platforms//cpu:x86_64"],
    ))
    checks.append(expect_equal("unknown constraints are empty", constraint_list("watch"), []))
    checks.append(expect_equal("device label", platform_label("device"), "//android/platforms:android_device"))
    checks.append(expect_equal("rust triples", rust_triples(), ["aarch64-linux-android", "x86_64-linux-android"]))
    checks.append(expect_equal("api floor", api_floor(), 31))
    checks.append(expect_equal("zero api means the floor", effective_api_level(0), 31))
    checks.append(expect_equal("explicit api wins", effective_api_level(34), 34))
    checks.append(expect_equal("device clang target", clang_target("device", 31), "aarch64-linux-android31"))
    checks.append(expect_equal("emulator clang target", clang_target("emulator", 34), "x86_64-linux-android34"))
    checks.append(expect_equal("unknown clang target is empty", clang_target("watch", 31), ""))
    checks.append(expect_equal("prebuilt dir", ndk_prebuilt_dir(), "toolchains/llvm/prebuilt/linux-x86_64"))
    checks.append(expect_equal(
        "sysroot api dir",
        sysroot_api_dir("aarch64-linux-android", 31),
        "sysroot/usr/lib/aarch64-linux-android/31",
    ))
    checks.append(expect_equal("supported tuple is clean", tuple_errors("device", 31), []))
    checks.append(expect_equal("zero api is clean", tuple_errors("emulator", 0), []))
    checks.append(expect_equal(
        "unknown tuple fails",
        tuple_errors("watch", 31),
        ["android: unknown tuple 'watch': want device, emulator"],
    ))
    checks.append(expect_equal(
        "downgraded api fails",
        tuple_errors("device", 29),
        ["android: api_level 29 is below 31"],
    ))
    checks.append(expect_equal("shared runtime is clean", cxx_runtime_errors("shared"), []))
    checks.append(expect_equal("static runtime is clean", cxx_runtime_errors("static"), []))
    checks.append(expect_equal("none runtime is clean", cxx_runtime_errors("none"), []))
    checks.append(expect_equal(
        "unknown runtime fails",
        cxx_runtime_errors("system"),
        ["android: cxx_runtime 'system' is unknown: want shared, static, none"],
    ))
    checks.append(expect_equal("pinned revision is clean", ndk_revision_errors("27.0.12077973"), []))
    checks.append(expect_equal("newer r27 revision is clean", ndk_revision_errors("27.2.12479018"), []))
    checks.append(expect_equal(
        "other lineage fails",
        ndk_revision_errors("26.3.11579264"),
        ["android: ndk revision '26.3.11579264' is outside r27"],
    ))
    checks.append(expect_equal("default config is clean", library_config_errors(default_library_config("emulator")), []))
    checks.append(expect_equal(
        "custom config is clean",
        library_config_errors(_config(tuple = "emulator", api_level = 34, features = ["neon"], cxx_runtime = "static")),
        [],
    ))
    checks.append(expect_equal(
        "bad tuple and runtime fail",
        library_config_errors(_config(tuple = "watch", cxx_runtime = "system")),
        [
            "android: unknown tuple 'watch': want device, emulator",
            "android: cxx_runtime 'system' is unknown: want shared, static, none",
        ],
    ))
    checks.append(expect_true("floor matches the sdk pin", api_floor() == 31))
    checks.append(expect_false("device and emulator share no triple", triple_for("device") == triple_for("emulator")))
    checks.append(expect_contains("device label names the package", platform_label("device"), "//android/platforms:"))
    return checks

def android_platform_tests(name):
    """Declares the Android native tuple inventory pin test."""
    starlark_test(
        name = name,
        mode = "load",
        checks = _platform_checks(),
    )
