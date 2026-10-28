"""Android acquisition pin and request coverage."""

load("//libs/starlark:defs.bzl", "expect_contains", "expect_equal", "expect_false", "expect_true", "starlark_test")
load(
    ":repos.bzl",
    "ANDROID_API_LEVEL",
    "ANDROID_NDK_RELEASE",
    "ANDROID_NDK_REVISION",
    "ANDROID_TARGET",
    "BUILD_TOOLS",
    "BUILD_TOOLS_ARCHIVE_PREFIX",
    "BUILD_TOOLS_ARCHIVE_SEPARATOR",
    "BUILD_TOOLS_ARTIFACTS",
    "BUNDLETOOL_REPOSITORY",
    "BUNDLETOOL_SHA256",
    "BUNDLETOOL_VERSION",
    "DEFAULT_MIRROR",
    "DEFAULT_PLATFORM_LEVEL",
    "EMULATOR_ARTIFACTS",
    "EMULATOR_BUILD",
    "HUB_NAME",
    "NDK_ARTIFACTS",
    "OPT_IN_PACKAGES",
    "PLATFORM_ARTIFACTS",
    "PLATFORM_CONSTRAINTS",
    "PLATFORM_TOOLS",
    "PLATFORM_TOOLS_ARCHIVE_PREFIX",
    "PLATFORM_TOOLS_ARTIFACTS",
    "REQUIRED_LICENSES",
    "SDK_PACKAGES",
    "SDK_PLATFORM",
    "artifact_errors",
    "build_tools_build",
    "build_tools_no_match_error",
    "build_tools_repositories",
    "build_tools_repository_name",
    "bundletool_url",
    "canonical_id_for",
    "default_request",
    "emulator_build",
    "emulator_repository_name",
    "fetches_payload",
    "hub_build",
    "ndk_build",
    "ndk_repositories",
    "ndk_repository_name",
    "no_match_error",
    "package_repositories",
    "platform_build",
    "platform_repository_name",
    "platform_tools_build",
    "platform_tools_repository_name",
    "request_errors",
    "url_for",
)

def _artifact(base, **kwargs):
    return struct(**{key: kwargs.get(key, getattr(base, key)) for key in [
        "anchor",
        "archive",
        "directory",
        "sha1",
        "sha256",
        "size",
    ]})

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
    checks.append(expect_equal("build tools", BUILD_TOOLS, "35.0.0"))
    checks.append(expect_equal("default platform level", DEFAULT_PLATFORM_LEVEL, 34))
    checks.append(expect_equal("platform-tools version", PLATFORM_TOOLS, "37.0.1"))
    checks.append(expect_equal("emulator build", EMULATOR_BUILD, "16428233"))
    checks.append(expect_equal("bundletool version", BUNDLETOOL_VERSION, "1.15.5"))
    checks.append(expect_equal("bundletool repository", BUNDLETOOL_REPOSITORY, "dx_android_bundletool"))
    checks.append(expect_true(
        "bundletool sha256 is 64 lowercase hex characters",
        len(BUNDLETOOL_SHA256) == 64 and len([c for c in BUNDLETOOL_SHA256.elems() if c not in "0123456789abcdef"]) == 0,
    ))
    checks.append(expect_equal(
        "bundletool url",
        bundletool_url(),
        "https://github.com/google/bundletool/releases/download/1.15.5/bundletool-all-1.15.5.jar",
    ))
    checks.append(expect_equal("hub name", HUB_NAME, "dx_android_sdk"))
    checks.append(expect_equal("required licenses", REQUIRED_LICENSES, ["android-sdk-license"]))
    checks.append(expect_equal("sdk packages", SDK_PACKAGES, [
        "platforms;android-34",
        "build-tools;35.0.0",
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
    checks.append(expect_equal("ndk sha256", artifact.sha256, "2f17eb8bcbfdc40201c0b36e9a70826fcd2524ab7a2a235e2c71186c302da1dc"))
    checks.append(expect_equal("ndk sha256 length", len(artifact.sha256), 64))
    checks.append(expect_equal(
        "short sha256 fails",
        artifact_errors({"linux_x86_64": _artifact(artifact, sha256 = artifact.sha256[1:])}),
        ["artifact 'linux_x86_64' sha256 is not 64 hex characters"],
    ))
    checks.append(expect_contains(
        "non-hex sha256 fails",
        "; ".join(artifact_errors({"linux_x86_64": _artifact(artifact, sha256 = "g" * 64)})),
        "sha256 is not 64 hex characters",
    ))
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
    tools = BUILD_TOOLS_ARTIFACTS["linux_x86_64"]
    checks.append(expect_equal("build-tools hosts", sorted(BUILD_TOOLS_ARTIFACTS.keys()), ["linux_x86_64"]))
    checks.append(expect_equal("build-tools archive prefix", BUILD_TOOLS_ARCHIVE_PREFIX, "build-tools_r35"))
    checks.append(expect_equal("build-tools archive", tools.archive, "build-tools_r35_linux.zip"))
    checks.append(expect_equal("build-tools directory", tools.directory, "android-15"))
    checks.append(expect_equal("build-tools anchor", tools.anchor, "source.properties"))
    checks.append(expect_equal("build-tools size", tools.size, 61958799))
    checks.append(expect_equal("build-tools sha1", tools.sha1, "2cfaa0bbb2336e9ec18ed3ecea84fa2e2af607bc"))
    checks.append(expect_equal("build-tools sha256", tools.sha256, "bd3a4966912eb8b30ed0d00b0cda6b6543b949d5ffe00bea54c04c81e1561d88"))
    checks.append(expect_equal("build-tools artifact errors", artifact_errors(BUILD_TOOLS_ARTIFACTS, BUILD_TOOLS_ARCHIVE_PREFIX, True, BUILD_TOOLS_ARCHIVE_SEPARATOR), []))
    checks.append(expect_equal(
        "build-tools short sha256 fails",
        artifact_errors({"linux_x86_64": _artifact(tools, sha256 = tools.sha256[1:])}, BUILD_TOOLS_ARCHIVE_PREFIX, True, BUILD_TOOLS_ARCHIVE_SEPARATOR),
        ["artifact 'linux_x86_64' sha256 is not 64 hex characters"],
    ))
    checks.append(expect_equal(
        "build-tools wrong archive fails",
        artifact_errors({"linux_x86_64": _artifact(tools, archive = "build-tools_r33_linux.zip")}, BUILD_TOOLS_ARCHIVE_PREFIX, True, BUILD_TOOLS_ARCHIVE_SEPARATOR),
        ["artifact 'linux_x86_64' archive is 'build-tools_r33_linux.zip', want the pinned 'build-tools_r35_linux.zip'"],
    ))
    checks.append(expect_equal(
        "build-tools archive with a dash separator is rejected",
        artifact_errors({"linux_x86_64": _artifact(tools, archive = "build-tools_r35-linux.zip")}, BUILD_TOOLS_ARCHIVE_PREFIX, True, BUILD_TOOLS_ARCHIVE_SEPARATOR),
        ["artifact 'linux_x86_64' archive is 'build-tools_r35-linux.zip', want the pinned 'build-tools_r35_linux.zip'"],
    ))
    checks.append(expect_equal("ndk keeps the dash separator", artifact_errors(), []))
    checks.append(expect_equal(
        "build-tools are not an ndk",
        artifact_errors(BUILD_TOOLS_ARTIFACTS),
        ["artifact 'linux_x86_64' archive is 'build-tools_r35_linux.zip', want the pinned 'android-ndk-r27-linux.zip'"],
    ))
    checks.append(expect_equal("build-tools repository", build_tools_repository_name("linux_x86_64"), "dx_android_build_tools_linux_x86_64"))
    checks.append(expect_equal("build-tools repositories", build_tools_repositories(), {"linux_x86_64": "dx_android_build_tools_linux_x86_64"}))
    checks.append(expect_equal(
        "build-tools cache identity",
        canonical_id_for(url_for(tools), "build-tools"),
        "dx-android-build-tools-archive:" + DEFAULT_MIRROR + "/build-tools_r35_linux.zip",
    ))
    checks.append(expect_contains("build-tools no-match names the version", build_tools_no_match_error(), "build-tools 35.0.0"))
    checks.append(expect_contains("build-tools no-match names the host", build_tools_no_match_error(), "linux_x86_64"))
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
        "pinned extra package fails",
        request_errors(_request(extra_packages = ["platforms;android-34"])),
        ["android_sdk: extra package 'platforms;android-34' must be one of platform-tools, emulator"],
    ))
    checks.append(expect_equal(
        "unknown extra package fails",
        request_errors(_request(extra_packages = ["anything"])),
        ["android_sdk: extra package 'anything' must be one of platform-tools, emulator"],
    ))
    checks.append(expect_equal(
        "repeated extra package fails",
        request_errors(_request(extra_packages = ["emulator", "emulator"])),
        ["android_sdk: extra package 'emulator' is listed twice"],
    ))
    checks.append(expect_equal("opt-in packages pass", request_errors(_request(extra_packages = ["platform-tools", "emulator"])), []))
    checks.append(expect_equal(
        "unpinned api level fails",
        request_errors(_request(api_level = 35)),
        ["android_sdk: api_level 35 is not pinned; want one of 31, 32, 33, 34"],
    ))
    checks.append(expect_equal(
        "downgraded api level fails in local mode",
        request_errors(_request(api_level = 29, local_path = "/opt/android-sdk")),
        ["android_sdk: api_level 29 is below 31"],
    ))
    checks.append(expect_equal("pinned api level passes", request_errors(_request(api_level = 32)), []))
    checks.append(expect_equal("default api level is zero", default_request().api_level, 0))
    checks.append(expect_equal("platform levels", sorted(PLATFORM_ARTIFACTS.keys()), [31, 32, 33, 34]))
    checks.append(expect_equal("platform artifact errors", artifact_errors(PLATFORM_ARTIFACTS, "", False), []))
    checks.append(expect_equal("platform-tools artifact errors", artifact_errors(PLATFORM_TOOLS_ARTIFACTS, PLATFORM_TOOLS_ARCHIVE_PREFIX), []))
    checks.append(expect_equal("emulator artifact errors", artifact_errors(EMULATOR_ARTIFACTS, ""), []))
    checks.append(expect_equal("platform-tools archive prefix", PLATFORM_TOOLS_ARCHIVE_PREFIX, "platform-tools_r37.0.1"))
    expected_platforms = {
        31: ("platform-31_r01.zip", "android-12", "ca5bcaa565cb37e9d287051d6dd0e49a5426ec29", "1d69fe1d7f9788d82ff3a374faf4f6ccc9d1d372aa84a86b5bcfb517523b0b3f", 56475526),
        32: ("platform-32_r01.zip", "android-12", "afae86ed55d29733d50996ffed832f2d1bd75b9a", "01d8da1c900e70fcf5da39767d5444e39928935b1a5927055ce749fc348ca7ae", 66108299),
        33: ("platform-33-ext3_r03.zip", "android-13", "394bc86d8d3452aa4d419b67743025a6fb2cd9d0", "b32b10f787867987f03ae8e101d217e053a9065b7136379fb353b388379aed1d", 67334237),
        34: ("platform-34-ext7_r03.zip", "android-34", "1f2e9478d6a7601425ceaa553311dc43191f103d", "16fdb74c55e59ae3ef52def135aec713508467bd56d7dabcd8c9be31fa8b20f3", 63180081),
    }
    for level in sorted(expected_platforms.keys()):
        want = expected_platforms[level]
        got = PLATFORM_ARTIFACTS[level]
        checks.append(expect_equal(
            "platform %d pin" % level,
            [got.anchor, got.archive, got.directory, got.sha1, got.sha256, got.size],
            ["android.jar"] + list(want),
        ))
    pt = PLATFORM_TOOLS_ARTIFACTS["linux_x86_64"]
    checks.append(expect_equal(
        "platform-tools pin",
        [pt.anchor, pt.archive, pt.directory, pt.sha1, pt.sha256, pt.size],
        ["adb", "platform-tools_r37.0.1-linux.zip", "platform-tools", "477254aa5f903c15cf51001717bdf347fb6b53e0", "d230f13842f60f782a8645f9c813f8f845bf36089ea7289f28c48f17979313f1", 9054187],
    ))
    em = EMULATOR_ARTIFACTS["linux_x86_64"]
    checks.append(expect_equal(
        "emulator pin",
        [em.anchor, em.archive, em.directory, em.sha1, em.sha256, em.size],
        ["emulator", "emulator-linux_x64-16428233.zip", "emulator", "cd7362ea55dfb86a418958138dc396e74165dd01", "b08fc43d8608d2955f607f1b287beb525041e730086bdca3af152accac3af9c1", 349654171],
    ))
    checks.append(expect_contains(
        "wrong platform archive fails",
        "; ".join(artifact_errors({31: _artifact(PLATFORM_ARTIFACTS[31], archive = "platform-32_r01.zip")}, "", False)),
        "want one starting with 'platform-31'",
    ))
    checks.append(expect_equal("platform repository", platform_repository_name(), "dx_android_platform"))
    checks.append(expect_equal("platform-tools repository", platform_tools_repository_name("linux_x86_64"), "dx_android_platform_tools_linux_x86_64"))
    checks.append(expect_equal("emulator repository", emulator_repository_name("linux_x86_64"), "dx_android_emulator_linux_x86_64"))
    checks.append(expect_equal("platform cache identity", canonical_id_for("u", "platform"), "dx-android-platform-archive:u"))
    checks.append(expect_equal("disabled request has no repositories", package_repositories(_request(enabled = False)), {}))
    default_repos = package_repositories(default_request())
    checks.append(expect_equal("default platform repository", str(default_repos["platform"]), '{34: "dx_android_platform"}'))
    checks.append(expect_false("default request skips opt-in packages", "emulator" in default_repos or "platform_tools" in default_repos))
    checks.append(expect_true("api_level changes what is fetched", package_repositories(_request(api_level = 31)) != default_repos))
    checks.append(expect_equal("api_level selects the platform", str(package_repositories(_request(api_level = 31))["platform"]), '{31: "dx_android_platform"}'))
    checks.append(expect_true(
        "extra_packages changes what is fetched",
        package_repositories(_request(extra_packages = ["platform-tools"])) != default_repos,
    ))
    checks.append(expect_equal(
        "platform-tools repositories",
        package_repositories(_request(extra_packages = ["platform-tools"]))["platform_tools"],
        {"linux_x86_64": "dx_android_platform_tools_linux_x86_64"},
    ))
    checks.append(expect_equal(
        "emulator repositories",
        package_repositories(_request(extra_packages = ["emulator"]))["emulator"],
        {"linux_x86_64": "dx_android_emulator_linux_x86_64"},
    ))
    local_repos = package_repositories(_request(local_path = "/opt/android-sdk"))
    checks.append(expect_true("local request differs from disabled", local_repos != package_repositories(_request(enabled = False))))
    checks.append(expect_equal("local ndk repository", local_repos["ndk"], {"linux_x86_64": "dx_android_sdk_local"}))
    checks.append(expect_true("local request differs from remote", local_repos != package_repositories(_request())))
    checks.append(expect_equal(
        "disabled hub selects nothing",
        hub_build({}, {}),
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
            "filegroup(",
            '    name = "zipalign",',
            "    srcs = [],",
            '    visibility = ["//visibility:public"],',
            ")",
            "",
            "filegroup(",
            '    name = "aapt",',
            "    srcs = [],",
            '    visibility = ["//visibility:public"],',
            ")",
            "",
            "filegroup(",
            '    name = "apksigner_jar",',
            "    srcs = [],",
            '    visibility = ["//visibility:public"],',
            ")",
            "",
            "filegroup(",
            '    name = "bundletool_jar",',
            "    srcs = [],",
            '    visibility = ["//visibility:public"],',
            ")",
            "",
            "filegroup(",
            '    name = "android_jar",',
            "    srcs = [],",
            '    visibility = ["//visibility:public"],',
            ")",
            "",
            "filegroup(",
            '    name = "adb",',
            "    srcs = [],",
            '    visibility = ["//visibility:public"],',
            ")",
            "",
            "filegroup(",
            '    name = "emulator",',
            "    srcs = [],",
            '    visibility = ["//visibility:public"],',
            ")",
            "",
        ]),
    ))
    enabled_hub = hub_build(
        {"linux_x86_64": "dx_android_ndk_linux_x86_64"},
        {"linux_x86_64": "dx_android_build_tools_linux_x86_64"},
        "dx_android_bundletool",
        platform_repo = "dx_android_platform",
    )
    checks.append(expect_contains("enabled hub aliases android_jar", enabled_hub, 'actual = "@dx_android_platform//:android_jar"'))
    checks.append(expect_false("enabled hub leaves adb empty", "@dx_android_platform_tools" in enabled_hub))
    checks.append(expect_contains("enabled hub keeps adb a target", enabled_hub, 'name = "adb"'))
    checks.append(expect_contains("enabled hub keeps emulator a target", enabled_hub, 'name = "emulator"'))
    opt_in_hub = hub_build(
        {},
        platform_tools_repos = {"linux_x86_64": "dx_android_platform_tools_linux_x86_64"},
        emulator_repos = {"linux_x86_64": "dx_android_emulator_linux_x86_64"},
    )
    checks.append(expect_contains("hub selects adb", opt_in_hub, '":linux_x86_64": "@dx_android_platform_tools_linux_x86_64//:adb"'))
    checks.append(expect_contains("hub selects emulator", opt_in_hub, '":linux_x86_64": "@dx_android_emulator_linux_x86_64//:emulator"'))
    checks.append(expect_contains("platform build exposes android.jar", platform_build("android.jar"), '"extracted/android.jar"'))
    checks.append(expect_contains("platform-tools build exposes adb", platform_tools_build("adb"), '"extracted/adb"'))
    checks.append(expect_contains("emulator build ships its tree", emulator_build("emulator"), 'glob(["extracted/**"])'))
    checks.append(expect_contains("enabled hub aliases the anchor", enabled_hub, "@dx_android_ndk_linux_x86_64//:ndk_anchor"))
    checks.append(expect_contains("enabled hub keeps the no-match guard", enabled_hub, "linux_x86_64"))
    checks.append(expect_contains("enabled hub aliases zipalign", enabled_hub, '":linux_x86_64": "@dx_android_build_tools_linux_x86_64//:zipalign"'))
    checks.append(expect_contains("enabled hub aliases aapt", enabled_hub, '":linux_x86_64": "@dx_android_build_tools_linux_x86_64//:aapt"'))
    checks.append(expect_contains("enabled hub aliases apksigner", enabled_hub, '":linux_x86_64": "@dx_android_build_tools_linux_x86_64//:apksigner_jar"'))
    checks.append(expect_contains("enabled hub guards build-tools", enabled_hub, build_tools_no_match_error()))
    checks.append(expect_contains("enabled hub aliases bundletool", enabled_hub, 'actual = "@dx_android_bundletool//file"'))
    tools_build = build_tools_build("source.properties")
    checks.append(expect_contains("build-tools build exposes zipalign", tools_build, '"extracted/zipalign"'))
    checks.append(expect_contains("build-tools build exposes aapt", tools_build, '"extracted/aapt"'))
    checks.append(expect_contains("build-tools build ships libc++", tools_build, '"extracted/lib64/libc++.so"'))
    checks.append(expect_contains("build-tools build exposes apksigner", tools_build, '"extracted/lib/apksigner.jar"'))
    checks.append(expect_contains("build-tools build exposes the anchor", tools_build, '"extracted/source.properties"'))
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
