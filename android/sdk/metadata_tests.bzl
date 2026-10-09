"""Android acquisition pin and request coverage."""

load("//libs/starlark:defs.bzl", "expect_contains", "expect_equal", "expect_false", "expect_true", "starlark_test")
load(
    ":repos.bzl",
    "ANDROID_API_LEVEL",
    "ANDROID_NDK_RELEASE",
    "ANDROID_NDK_REVISION",
    "ANDROID_TARGET",
    "BUILD_TOOLS",
    "CMDLINE_TOOLS",
    "DEFAULT_MIRROR",
    "HUB_NAME",
    "MIN_JDK_VERSION",
    "NDK_ARTIFACTS",
    "OPT_IN_PACKAGES",
    "PLATFORM_CONSTRAINTS",
    "REQUIRED_LICENSES",
    "SDK_PACKAGES",
    "SDK_PLATFORM",
    "artifact_errors",
    "canonical_id_for",
    "default_request",
    "fetches_payload",
    "hub_build",
    "ndk_build",
    "ndk_repositories",
    "ndk_repository_name",
    "no_match_error",
    "request_errors",
    "url_for",
)

def _request(**kwargs):
    base = struct(
        accept_licenses = list(REQUIRED_LICENSES),
        api_level = ANDROID_API_LEVEL,
        enabled = True,
        extra_packages = [],
        local_path = "",
        mirror = "",
    )
    return struct(**{key: kwargs.get(key, getattr(base, key)) for key in [
        "accept_licenses",
        "api_level",
        "enabled",
        "extra_packages",
        "local_path",
        "mirror",
    ]})

def _android_checks():
    checks = []
    checks.append(expect_equal("ndk revision", ANDROID_NDK_REVISION, "27.0.12077973"))
    checks.append(expect_equal("ndk release", ANDROID_NDK_RELEASE, "r27"))
    checks.append(expect_equal("api level", ANDROID_API_LEVEL, 31))
    checks.append(expect_equal("target", ANDROID_TARGET, "aarch64-linux-android"))
    checks.append(expect_equal("platform", SDK_PLATFORM, "android-34"))
    checks.append(expect_equal("build tools", BUILD_TOOLS, "34.0.0"))
    checks.append(expect_equal("command line tools", CMDLINE_TOOLS, "13.0"))
    checks.append(expect_equal("minimum jdk", MIN_JDK_VERSION, 17))
    checks.append(expect_equal("hub name", HUB_NAME, "dx_android_sdk"))
    checks.append(expect_equal("required licenses", REQUIRED_LICENSES, ["android-sdk-license"]))
    checks.append(expect_equal("sdk packages", SDK_PACKAGES, [
        "platforms;android-34",
        "build-tools;34.0.0",
        "ndk;27.0.12077973",
    ]))
    checks.append(expect_equal("opt-in packages", OPT_IN_PACKAGES, ["platform-tools", "emulator"]))
    checks.append(expect_equal("supported hosts", sorted(NDK_ARTIFACTS.keys()), ["linux_x86_64"]))
    checks.append(expect_equal("artifact errors", artifact_errors(), []))
    checks.append(expect_equal("ndk repositories", ndk_repositories(), {"linux_x86_64": "dx_android_ndk_linux_x86_64"}))
    artifact = NDK_ARTIFACTS["linux_x86_64"]
    checks.append(expect_equal("ndk repository", ndk_repository_name("linux_x86_64"), "dx_android_ndk_linux_x86_64"))
    checks.append(expect_equal("ndk archive", artifact.archive, "android-ndk-r27-linux.zip"))
    checks.append(expect_equal("ndk directory", artifact.directory, "android-ndk-r27"))
    checks.append(expect_equal("ndk anchor", artifact.anchor, "source.properties"))
    checks.append(expect_equal("ndk size", artifact.size, 663957918))
    checks.append(expect_equal("ndk sha1", artifact.sha1, "5e5cd517bdb98d7e0faf2c494a3041291e71bdcc"))
    checks.append(expect_equal("ndk sha1 length", len(artifact.sha1), 40))
    checks.append(expect_true("ndk https url", url_for(artifact).startswith("https://")))
    checks.append(expect_equal(
        "ndk default url",
        url_for(artifact),
        DEFAULT_MIRROR + "/android-ndk-r27-linux.zip",
    ))
    checks.append(expect_equal(
        "ndk mirror url",
        url_for(artifact, "https://mirror.example.com/android"),
        "https://mirror.example.com/android/android-ndk-r27-linux.zip",
    ))
    checks.append(expect_equal(
        "ndk cache identity",
        canonical_id_for(url_for(artifact)),
        "dx-android-ndk-archive:" + DEFAULT_MIRROR + "/android-ndk-r27-linux.zip",
    ))
    checks.append(expect_equal(
        "ndk constraints",
        PLATFORM_CONSTRAINTS["linux_x86_64"],
        ["@platforms//os:linux", "@platforms//cpu:x86_64"],
    ))
    checks.append(expect_contains("no-match names the qualified host", no_match_error(), "linux_x86_64"))
    checks.append(expect_contains("no-match names the release", no_match_error(), "r27"))
    checks.append(expect_equal("default request is clean", request_errors(default_request()), []))
    checks.append(expect_false("no request fetches nothing", fetches_payload([])))
    checks.append(expect_false("disabled request fetches nothing", fetches_payload([_request(enabled = False)])))
    checks.append(expect_false("installed request fetches nothing", fetches_payload([_request(local_path = "/opt/android-sdk")])))
    checks.append(expect_true("default request fetches", fetches_payload([default_request()])))
    checks.append(expect_equal(
        "missing license fails",
        request_errors(_request(accept_licenses = [])),
        ["android_sdk: accept android-sdk-license before acquisition"],
    ))
    checks.append(expect_equal(
        "plain http mirror fails",
        request_errors(_request(mirror = "http://mirror.example.com/android")),
        ["android_sdk: mirror 'http://mirror.example.com/android' must use https"],
    ))
    checks.append(expect_equal(
        "downgraded api level fails",
        request_errors(_request(api_level = 29)),
        ["android_sdk: api_level 29 is below 31"],
    ))
    checks.append(expect_equal(
        "duplicate extra package fails",
        request_errors(_request(extra_packages = ["platforms;android-34"])),
        ["android_sdk: extra package 'platforms;android-34' is empty or already pinned"],
    ))
    checks.append(expect_equal(
        "disabled hub selects nothing",
        hub_build({}),
        "\n".join([
            "config_setting(",
            '    name = "linux_x86_64",',
            "    constraint_values = [",
            '        "@platforms//os:linux",',
            '        "@platforms//cpu:x86_64",',
            "    ],",
            ")",
            "",
            "filegroup(",
            '    name = "ndk",',
            "    srcs = [],",
            '    visibility = ["//visibility:public"],',
            ")",
            "",
        ]),
    ))
    enabled_hub = hub_build({"linux_x86_64": "dx_android_ndk_linux_x86_64"})
    checks.append(expect_contains("enabled hub aliases the anchor", enabled_hub, "@dx_android_ndk_linux_x86_64//:ndk_anchor"))
    checks.append(expect_contains("enabled hub keeps the no-match guard", enabled_hub, "linux_x86_64"))
    anchor_build = ndk_build("source.properties")
    checks.append(expect_contains("ndk build exposes the anchor", anchor_build, "extracted/source.properties"))
    return checks

def android_metadata_tests(name):
    """Declares the Android acquisition inventory pin test."""
    starlark_test(
        name = name,
        mode = "load",
        checks = _android_checks(),
    )
