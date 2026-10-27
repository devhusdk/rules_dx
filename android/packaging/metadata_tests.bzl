"""Application packaging selection coverage."""

load("//libs/starlark:defs.bzl", "expect_equal", "expect_true", "starlark_test")
load(":defs.bzl", "aab_check_errors", "aab_sign_errors", "apk_manifest_errors", "release_sign_errors", "rust_android_apk_errors")
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
    "abi_platforms",
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
        {"arm64-v8a": str(Label("//android/platforms:arm64-v8a")), "x86_64": str(Label("//android/platforms:x86_64"))},
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
    checks.append(expect_equal("manifest lib name", manifest_lib_name("app_apk"), "app_apk_bin"))
    checks.append(expect_equal(
        "abi platforms keep order",
        abi_platforms(["arm64-v8a", "x86_64"]),
        [str(Label("//android/platforms:arm64-v8a")), str(Label("//android/platforms:x86_64"))],
    ))
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
    checks.append(expect_equal("default sign errors", release_sign_errors(["v1", "v2", "v3"], 0), []))
    checks.append(expect_equal("min sdk sign errors", release_sign_errors(["v2"], 31), []))
    checks.append(expect_equal(
        "unknown scheme sign errors",
        release_sign_errors(["v9"], 0),
        ["android_apk_release_sign: scheme 'v9' is unknown: want v1, v2, v3, v4"],
    ))
    checks.append(expect_equal(
        "empty scheme sign errors",
        release_sign_errors([], 0),
        ["android_apk_release_sign: schemes must name at least one signature scheme"],
    ))
    checks.append(expect_equal(
        "duplicate scheme sign errors",
        release_sign_errors(["v2", "v2"], 0),
        ["android_apk_release_sign: schemes holds duplicates"],
    ))
    checks.append(expect_equal(
        "v4 alone sign errors",
        release_sign_errors(["v4"], 0),
        ["android_apk_release_sign: scheme v4 needs v2 or v3"],
    ))
    checks.append(expect_equal(
        "v1 and v4 sign errors",
        release_sign_errors(["v1", "v4"], 0),
        ["android_apk_release_sign: scheme v4 needs v2 or v3"],
    ))
    checks.append(expect_equal("v2 and v4 sign errors", release_sign_errors(["v2", "v4"], 0), []))
    checks.append(expect_equal("v3 and v4 sign errors", release_sign_errors(["v3", "v4"], 0), []))
    checks.append(expect_equal(
        "unknown scheme with v4 sign errors",
        release_sign_errors(["v9", "v4"], 0),
        [
            "android_apk_release_sign: scheme 'v9' is unknown: want v1, v2, v3, v4",
            "android_apk_release_sign: scheme v4 needs v2 or v3",
        ],
    ))
    checks.append(expect_equal(
        "low min sdk sign errors",
        release_sign_errors(["v2"], 21),
        ["android_apk_release_sign: min_sdk 21 is below 31"],
    ))
    checks.append(expect_equal("clean aab check", aab_check_errors(["arm64-v8a", "x86_64"], ["string/app_name"]), []))
    checks.append(expect_equal(
        "unknown abi aab check",
        aab_check_errors(["mips"], []),
        ["rust_android_apk: abi 'mips' is unknown: want arm64-v8a, x86_64"],
    ))
    checks.append(expect_equal(
        "malformed resource aab check",
        aab_check_errors([], ["app_name", "string/", "/app_name", "a/b/c"]),
        [
            "android_aab_check: resource 'app_name' must be type/name",
            "android_aab_check: resource 'string/' must be type/name",
            "android_aab_check: resource '/app_name' must be type/name",
            "android_aab_check: resource 'a/b/c' must be type/name",
        ],
    ))
    checks.append(expect_equal(
        "duplicate abi aab check",
        aab_check_errors(["x86_64", "x86_64"], []),
        ["rust_android_apk: abis holds duplicates"],
    ))
    checks.append(expect_equal("one aab sign", aab_sign_errors("//:app", 1), []))
    checks.append(expect_equal(
        "no aab sign",
        aab_sign_errors("//:app", 0),
        ["android_aab_release_sign: '//:app' built 0 aab files, want exactly one"],
    ))
    return checks

def android_packaging_tests(name):
    """Instantiates the application packaging selection tests."""
    starlark_test(
        name = name,
        mode = "load",
        checks = _packaging_checks(),
    )
