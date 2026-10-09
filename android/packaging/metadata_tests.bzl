"""Application packaging selection coverage."""

load("//libs/starlark:defs.bzl", "expect_equal", "expect_true", "starlark_test")
load(":defs.bzl", "apk_manifest_errors", "rust_android_apk_errors")
load(
    ":repos.bzl",
    "ABI_PLATFORMS",
    "APP_SDK_API_LEVEL",
    "APP_SDK_BUILD_TOOLS",
    "APP_SDK_LICENSES",
    "APP_SDK_PLATFORM",
    "APP_SDK_REPOSITORY",
    "KNOWN_ABIS",
    "TUPLE_ABIS",
    "abi_errors",
    "abis_for",
    "app_id_errors",
    "manifest_lib_name",
    "sdk_request_errors",
)

def _packaging_checks():
    checks = []
    checks.append(expect_equal("sdk repository", APP_SDK_REPOSITORY, "androidsdk"))
    checks.append(expect_equal("sdk platform", APP_SDK_PLATFORM, "android-34"))
    checks.append(expect_equal("sdk build tools", APP_SDK_BUILD_TOOLS, "34.0.0"))
    checks.append(expect_equal("sdk api level", APP_SDK_API_LEVEL, 31))
    checks.append(expect_equal("sdk licenses", APP_SDK_LICENSES, ["android-sdk-license"]))
    checks.append(expect_equal("known abis", KNOWN_ABIS, ["arm64-v8a", "x86_64"]))
    checks.append(expect_equal("device abi", TUPLE_ABIS["device"], "arm64-v8a"))
    checks.append(expect_equal("emulator abi", TUPLE_ABIS["emulator"], "x86_64"))
    checks.append(expect_equal(
        "abi platforms",
        ABI_PLATFORMS,
        {"arm64-v8a": "//android/platforms:android_device", "x86_64": "//android/platforms:android_emulator"},
    ))
    checks.append(expect_equal("abis for tuples", abis_for(["device", "emulator"]), ["arm64-v8a", "x86_64"]))
    checks.append(expect_equal("abis skip unknown tuples", abis_for(["device", "watch"]), ["arm64-v8a"]))
    checks.append(expect_equal("known abi errors", abi_errors(["arm64-v8a", "x86_64"]), []))
    checks.append(expect_equal(
        "unknown abi errors",
        abi_errors(["mips"]),
        ["rust_android_apk: abi 'mips' is unknown: want arm64-v8a, x86_64"],
    ))
    checks.append(expect_equal(
        "duplicate abi errors",
        abi_errors(["arm64-v8a", "arm64-v8a"]),
        ["rust_android_apk: abis holds duplicates"],
    ))
    checks.append(expect_equal("valid app id errors", app_id_errors("com.example.app"), []))
    checks.append(expect_equal(
        "short app id errors",
        app_id_errors("app"),
        ["rust_android_apk: app_id 'app' needs at least two dot-separated parts"],
    ))
    checks.append(expect_equal(
        "empty part app id errors",
        app_id_errors("com..app"),
        ["rust_android_apk: app_id 'com..app' holds an empty part"],
    ))
    checks.append(expect_equal(
        "leading digit app id errors",
        app_id_errors("com.1example.app"),
        ["rust_android_apk: app_id part '1example' must start with a letter"],
    ))
    checks.append(expect_equal(
        "dash app id errors",
        app_id_errors("com.exa-mple.app"),
        ["rust_android_apk: app_id part 'exa-mple' holds '-'"],
    ))
    checks.append(expect_equal("local sdk request", sdk_request_errors("/sdk", ["android-sdk-license"]), []))
    checks.append(expect_equal(
        "unlicensed sdk request",
        sdk_request_errors("/sdk", []),
        ["android_app_sdk: accept android-sdk-license before packaging"],
    ))
    checks.append(expect_equal("unset sdk request", sdk_request_errors("", []), []))
    checks.append(expect_equal("manifest lib name", manifest_lib_name("app_apk"), "app_apk"))
    checks.append(expect_equal(
        "valid apk errors",
        rust_android_apk_errors("app_apk", "com.example.app", ["arm64-v8a"], 31, 1),
        [],
    ))
    checks.append(expect_true(
        "apk errors name app id abis api and version",
        len(rust_android_apk_errors("", "app", ["mips"], 29, 0)) == 5,
    ))
    checks.append(expect_equal(
        "valid manifest errors",
        apk_manifest_errors("com.example.app", 31, 1, "android.app.NativeActivity"),
        [],
    ))
    checks.append(expect_true(
        "manifest errors app id api version and activity",
        len(apk_manifest_errors("app", 29, 0, "")) == 4,
    ))
    return checks

def android_packaging_tests(name):
    """Instantiates the application packaging selection tests."""
    starlark_test(
        name = name,
        mode = "load",
        checks = _packaging_checks(),
    )
