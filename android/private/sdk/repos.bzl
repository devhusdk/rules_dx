"""Opt-in Android NDK and build-tools acquisition."""

load("@dx_pins_data//:pins.bzl", "DATA")
load("//core:cells.bzl", "unsupported_constraint")
load("//core:host.bzl", "host_cell")

ANDROID_NDK_REVISION = "27.0.12077973"

ANDROID_NDK_RELEASE = "r27"

ANDROID_NDK_CLANG_VERSION = "18"

NDK_PREBUILT = {
    "linux_x86_64": "toolchains/llvm/prebuilt/linux-x86_64",
    "macos_arm64": "toolchains/llvm/prebuilt/darwin-x86_64",
    "macos_x86_64": "toolchains/llvm/prebuilt/darwin-x86_64",
    "windows_arm64": "toolchains/llvm/prebuilt/windows-x86_64",
    "windows_x86_64": "toolchains/llvm/prebuilt/windows-x86_64",
}

NDK_TOOL_SUFFIX = {
    "linux_x86_64": "",
    "macos_arm64": "",
    "macos_x86_64": "",
    "windows_arm64": ".exe",
    "windows_x86_64": ".exe",
}

NDK_TRIPLES = ["aarch64-linux-android", "x86_64-linux-android"]

ANDROID_API_LEVEL = 31

ANDROID_TARGET = "aarch64-linux-android"

DEFAULT_PLATFORM_LEVEL = 34

SDK_PLATFORM = "android-%d" % DEFAULT_PLATFORM_LEVEL

BUILD_TOOLS = "35.0.0"

PLATFORM_TOOLS = "37.0.1"

EMULATOR_BUILD = "16428233"

BUNDLETOOL_VERSION = "1.15.5"

BUNDLETOOL_SHA256 = "0ebac88764e16b2154aa7506187917d169959338ad9d510e3174bcc96c9d0f40"

BUNDLETOOL_REPOSITORY = "dx_android_bundletool"

DEFAULT_MIRROR = "https://dl.google.com/android/repository"

REQUIRED_LICENSES = ["android-sdk-license"]

SDK_PACKAGES = [
    "platforms;android-34",
    "build-tools;35.0.0",
    "ndk;27.0.12077973",
]

OPT_IN_PACKAGES = [
    "platform-tools",
    "emulator",
]

_PINS = DATA["dx_android"]["archives"]

# windows_arm64 runs the x64 NDK under emulation; Google ships no arm64 Windows NDK.
NDK_ARTIFACTS = {
    "linux_x86_64": struct(
        anchor = "source.properties",
        archive = "android-ndk-r27-linux.zip",
        directory = "android-ndk-r27",
        sha1 = "5e5cd517bdb98d7e0faf2c494a3041291e71bdcc",
        sha256 = _PINS["android-ndk-r27-linux.zip"]["sha256"],
        size = 663957918,
    ),
    "macos_arm64": struct(
        anchor = "source.properties",
        archive = "android-ndk-r27-darwin.zip",
        directory = "android-ndk-r27",
        sha1 = "1a4a8c39c018430ff170657fc1673d895f5e4570",
        sha256 = _PINS["android-ndk-r27-darwin.zip"]["sha256"],
        size = 836135405,
    ),
    "macos_x86_64": struct(
        anchor = "source.properties",
        archive = "android-ndk-r27-darwin.zip",
        directory = "android-ndk-r27",
        sha1 = "1a4a8c39c018430ff170657fc1673d895f5e4570",
        sha256 = _PINS["android-ndk-r27-darwin.zip"]["sha256"],
        size = 836135405,
    ),
    "windows_arm64": struct(
        anchor = "source.properties",
        archive = "android-ndk-r27-windows.zip",
        directory = "android-ndk-r27",
        sha1 = "0ea2756e6815356831bda3af358cce4cdb6a981e",
        sha256 = _PINS["android-ndk-r27-windows.zip"]["sha256"],
        size = 781482169,
    ),
    "windows_x86_64": struct(
        anchor = "source.properties",
        archive = "android-ndk-r27-windows.zip",
        directory = "android-ndk-r27",
        sha1 = "0ea2756e6815356831bda3af358cce4cdb6a981e",
        sha256 = _PINS["android-ndk-r27-windows.zip"]["sha256"],
        size = 781482169,
    ),
}

BUILD_TOOLS_ARTIFACTS = {
    "linux_x86_64": struct(
        anchor = "source.properties",
        archive = "build-tools_r35_linux.zip",
        directory = "android-15",
        sha1 = "2cfaa0bbb2336e9ec18ed3ecea84fa2e2af607bc",
        sha256 = _PINS["build-tools_r35_linux.zip"]["sha256"],
        size = 61958799,
    ),
    "macos_arm64": struct(
        anchor = "source.properties",
        archive = "build-tools_r35_macosx.zip",
        directory = "android-15",
        sha1 = "93ab8ce91230e067b5add4bfa79919c52b27f072",
        sha256 = _PINS["build-tools_r35_macosx.zip"]["sha256"],
        size = 76857898,
    ),
    "windows_x86_64": struct(
        anchor = "source.properties",
        archive = "build-tools_r35_windows.zip",
        directory = "android-15",
        sha1 = "af059bb67cf7786f45ee0db85e2d24985df1b4b6",
        sha256 = _PINS["build-tools_r35_windows.zip"]["sha256"],
        size = 59878107,
    ),
}

PLATFORM_ARTIFACTS = {
    31: struct(
        anchor = "android.jar",
        archive = "platform-31_r01.zip",
        directory = "android-12",
        sha1 = "ca5bcaa565cb37e9d287051d6dd0e49a5426ec29",
        sha256 = _PINS["platform-31_r01.zip"]["sha256"],
        size = 56475526,
    ),
    32: struct(
        anchor = "android.jar",
        archive = "platform-32_r01.zip",
        directory = "android-12",
        sha1 = "afae86ed55d29733d50996ffed832f2d1bd75b9a",
        sha256 = _PINS["platform-32_r01.zip"]["sha256"],
        size = 66108299,
    ),
    33: struct(
        anchor = "android.jar",
        archive = "platform-33-ext3_r03.zip",
        directory = "android-13",
        sha1 = "394bc86d8d3452aa4d419b67743025a6fb2cd9d0",
        sha256 = _PINS["platform-33-ext3_r03.zip"]["sha256"],
        size = 67334237,
    ),
    34: struct(
        anchor = "android.jar",
        archive = "platform-34-ext7_r03.zip",
        directory = "android-34",
        sha1 = "1f2e9478d6a7601425ceaa553311dc43191f103d",
        sha256 = _PINS["platform-34-ext7_r03.zip"]["sha256"],
        size = 63180081,
    ),
}

PLATFORM_TOOLS_ARTIFACTS = {
    "linux_x86_64": struct(
        anchor = "adb",
        archive = "platform-tools_r37.0.1-linux.zip",
        directory = "platform-tools",
        sha1 = "477254aa5f903c15cf51001717bdf347fb6b53e0",
        sha256 = _PINS["platform-tools_r37.0.1-linux.zip"]["sha256"],
        size = 9054187,
    ),
    "macos_arm64": struct(
        anchor = "adb",
        archive = "platform-tools_r37.0.1-darwin.zip",
        directory = "platform-tools",
        sha1 = "6ae73f4de6452dc57e62ec02b68eed92a4c21661",
        sha256 = _PINS["platform-tools_r37.0.1-darwin.zip"]["sha256"],
        size = 16110554,
    ),
    "windows_x86_64": struct(
        anchor = "adb.exe",
        archive = "platform-tools_r37.0.1-win.zip",
        directory = "platform-tools",
        sha1 = "e03e78b1d80b396f1c3358e31251cb31740e1110",
        sha256 = _PINS["platform-tools_r37.0.1-win.zip"]["sha256"],
        size = 8044989,
    ),
}

EMULATOR_ARTIFACTS = {
    "linux_x86_64": struct(
        anchor = "emulator",
        archive = "emulator-linux_x64-16428233.zip",
        directory = "emulator",
        sha1 = "cd7362ea55dfb86a418958138dc396e74165dd01",
        sha256 = _PINS["emulator-linux_x64-16428233.zip"]["sha256"],
        size = 349654171,
    ),
}

NDK_ARCHIVE_PREFIX = "android-ndk-" + ANDROID_NDK_RELEASE

BUILD_TOOLS_ARCHIVE_PREFIX = "build-tools_r" + BUILD_TOOLS.split(".")[0]

BUILD_TOOLS_ARCHIVE_SEPARATOR = "_"

PLATFORM_TOOLS_ARCHIVE_PREFIX = "platform-tools_r" + PLATFORM_TOOLS

# Host cell to the host whose build-tools and platform-tools it runs.
# windows_arm64 runs the x64 archives under emulation.
SDK_HOSTS = {
    "linux_x86_64": "linux_x86_64",
    "macos_arm64": "macos_arm64",
    "windows_arm64": "windows_x86_64",
    "windows_x86_64": "windows_x86_64",
}

PLATFORM_CONSTRAINTS = {
    "linux_x86_64": ["@platforms//os:linux", "@platforms//cpu:x86_64"],
    "macos_arm64": ["@platforms//os:macos", "@platforms//cpu:arm64"],
    "macos_x86_64": ["@platforms//os:macos", "@platforms//cpu:x86_64"],
    "windows_arm64": ["@platforms//os:windows", "@platforms//cpu:arm64"],
    "windows_x86_64": ["@platforms//os:windows", "@platforms//cpu:x86_64"],
}

# Hosts Google ships no SDK for; every hub alias resolves to an incompatible target there.
SDK_UNSUPPORTED_HOSTS = {
    "linux_arm64": ["@platforms//os:linux", "@platforms//cpu:arm64"],
}

_NO_SDK_HOST_COMPATIBLE_WITH = [str(Label("@platforms//:incompatible")), str(unsupported_constraint("android"))]

# Libraries each build-tools binary loads from the archive; the Windows tools need only the system CRT.
BUILD_TOOLS_LIBS = {
    "linux_x86_64": ["lib64/libc++.so", "lib64/libc++.so.1"],
    "macos_arm64": ["lib64/libc++.dylib"],
    "windows_x86_64": [],
}

# Libraries adb loads from the archive.
PLATFORM_TOOLS_LIBS = {
    "linux_x86_64": [],
    "macos_arm64": [],
    "windows_x86_64": ["AdbWinApi.dll", "AdbWinUsbApi.dll"],
}

HUB_NAME = "dx_android_sdk"

APP_SDK_HUB_NAME = "dx_androidsdk"

APP_SDK_FILES_NAME = APP_SDK_HUB_NAME + "_files"

APP_SDK_FORWARDED = [
    "sdk",
    "sdk-%d",
    "sdk-dummy",
    "files",
    "fail",
    "d8_jar_import",
    "aapt_binary",
    "aapt2",
    "aapt2_binary",
    "aidl_binary",
    "zipalign",
    "zipalign_binary",
    "apksigner",
    "core-for-system-modules-jar",
    "org_apache_http_legacy",
    "org_apache_http_legacy-%d",
    "legacy_test-%d",
    "android_car",
    "android_car-%d",
    "adb",
    "platform-tools/adb",
    "dexdump",
    "emulator",
    "sdk_path",
    "build_tools_libs",
    "d8_compat_dx",
    "dx_binary",
    "main_dex_list_creator",
    "generate_main_dex_list",
]

_NDK_REPOSITORY_PREFIX = "dx_android_ndk_"

_BUILD_TOOLS_REPOSITORY_PREFIX = "dx_android_build_tools_"

_PLATFORM_REPOSITORY = "dx_android_platform"

_PLATFORM_TOOLS_REPOSITORY_PREFIX = "dx_android_platform_tools_"

_EMULATOR_REPOSITORY_PREFIX = "dx_android_emulator_"

_ARTIFACT_KEYS = [
    "anchor",
    "archive",
    "directory",
    "sha1",
    "sha256",
    "size",
]

_HEX = "0123456789abcdef"

def ndk_repository_name(platform):
    """Returns the repository name one platform's NDK is acquired into."""
    return _NDK_REPOSITORY_PREFIX + platform

def ndk_repositories():
    """Returns the platform-keyed NDK repository names."""
    return {platform: ndk_repository_name(platform) for platform in sorted(NDK_ARTIFACTS.keys())}

def build_tools_repository_name(platform):
    """Returns the repository name one platform's build-tools are acquired into."""
    return _BUILD_TOOLS_REPOSITORY_PREFIX + platform

def build_tools_repositories():
    """Returns the host-keyed build-tools repository names."""
    return {host: build_tools_repository_name(SDK_HOSTS[host]) for host in sorted(SDK_HOSTS.keys())}

def platform_repository_name():
    """Returns the repository name the requested platform is acquired into."""
    return _PLATFORM_REPOSITORY

def platform_tools_repository_name(host):
    """Returns the repository name one host's platform-tools are acquired into."""
    return _PLATFORM_TOOLS_REPOSITORY_PREFIX + host

def platform_tools_repositories():
    """Returns the host-keyed platform-tools repository names."""
    return {host: platform_tools_repository_name(SDK_HOSTS[host]) for host in sorted(SDK_HOSTS.keys())}

def emulator_repository_name(host):
    """Returns the repository name one host's emulator is acquired into."""
    return _EMULATOR_REPOSITORY_PREFIX + host

def bundletool_url():
    """Returns the download URL of the pinned bundletool jar."""
    return ("https://github.com/google/bundletool/releases/download/" + BUNDLETOOL_VERSION +
            "/bundletool-all-" + BUNDLETOOL_VERSION + ".jar")

def no_match_error():
    """Returns the diagnostic for an execution platform with no qualified NDK."""
    return ("rules_dx: no pinned Android NDK " + ANDROID_NDK_RELEASE + " artifact for this execution " +
            "platform; want one of " + ", ".join(sorted(NDK_ARTIFACTS.keys())))

def build_tools_no_match_error():
    """Returns the diagnostic for a platform with no qualified build-tools."""
    return ("rules_dx: no pinned Android build-tools " + BUILD_TOOLS + " artifact for this " +
            "platform; want one of " + ", ".join(sorted(SDK_HOSTS.keys())))

def artifact_errors(artifacts = NDK_ARTIFACTS, prefix = NDK_ARCHIVE_PREFIX, hosted = True, separator = "-"):
    """Returns one error string per pinned artifact that is not ready to download."""
    errors = []
    for key in sorted(artifacts.keys()):
        platform = str(key)
        if hosted and key not in PLATFORM_CONSTRAINTS:
            errors.append("artifact '" + platform + "' names no platform constraints")
        artifact = artifacts[key]
        missing = [name for name in _ARTIFACT_KEYS if getattr(artifact, name, "") == ""]
        if len(missing) > 0:
            errors.append("artifact '" + platform + "' is missing " + ", ".join(missing))
            continue
        if not hosted:
            expected = "platform-" + platform
            if not artifact.archive.startswith(expected):
                errors.append("artifact '" + platform + "' archive is '" + artifact.archive +
                              "', want one starting with '" + expected + "'")
        elif prefix != "":
            expected = prefix + separator + _release_suffix(platform, prefix)
            if artifact.archive != expected:
                errors.append("artifact '" + platform + "' archive is '" + artifact.archive +
                              "', want the pinned '" + expected + "'")
        if len(artifact.sha1) != 40:
            errors.append("artifact '" + platform + "' sha1 is not 40 hex characters")
        if len(artifact.sha256) != 64 or len([c for c in artifact.sha256.elems() if c not in _HEX]) > 0:
            errors.append("artifact '" + platform + "' sha256 is not 64 hex characters")
        if artifact.size <= 0:
            errors.append("artifact '" + platform + "' size is not positive")
    return errors

def _release_suffix(platform, prefix):
    """Returns the upstream file suffix for one qualified execution platform."""
    parts = platform.split("_", 1)
    if parts[0] == "linux":
        return "linux.zip"
    if parts[0] == "macos":
        return "macosx.zip" if prefix == BUILD_TOOLS_ARCHIVE_PREFIX else "darwin.zip"
    if parts[0] == "windows" and prefix == PLATFORM_TOOLS_ARCHIVE_PREFIX:
        return "win.zip"
    return parts[0] + ".zip"

def url_for(artifact, mirror = ""):
    """Returns the download URL one pinned artifact lives at."""
    base = mirror if mirror != "" else DEFAULT_MIRROR
    return base + "/" + artifact.archive

def canonical_id_for(url, kind = "ndk"):
    """Returns the stable cache identity for one pinned download URL."""
    return "dx-android-" + kind + "-archive:" + url

def default_request():
    """Returns the opt-in request enabling the qualified host NDK."""
    return struct(
        accept_licenses = list(REQUIRED_LICENSES),
        api_level = 0,
        enabled = True,
        extra_packages = [],
        mirror = "",
    )

def select_request(root_requests, other_requests, default):
    """Returns struct(request, error): the root tag, else default plus every other module's extra_packages."""
    if len(root_requests) > 1:
        return struct(request = None, error = "android: configure the Android SDK once")
    if len(root_requests) == 1:
        return struct(request = root_requests[0], error = "")
    packages = list(default.extra_packages)
    for request in other_requests:
        packages.extend([name for name in request.extra_packages if name not in packages])
    merged = struct(
        accept_licenses = default.accept_licenses,
        api_level = default.api_level,
        enabled = default.enabled,
        extra_packages = packages,
        mirror = default.mirror,
    )
    return struct(request = merged, error = "")

def license_errors(request):
    """Returns the error for each required license the request leaves unaccepted."""
    missing = [name for name in REQUIRED_LICENSES if name not in request.accept_licenses]
    if len(missing) == 0:
        return []
    quoted = ['"' + name + '"' for name in missing]
    return [
        "android: accept " + ", ".join(missing) + " before acquisition; " +
        "add dx_licenses.accept(ids = [" + ", ".join(quoted) + "]) to the root MODULE.bazel",
    ]

def request_errors(request):
    """Returns one error string per rejected field of one acquisition request."""
    errors = []
    if not request.enabled:
        return errors
    if request.api_level != 0 and request.api_level < ANDROID_API_LEVEL:
        errors.append("android: api_level " + str(request.api_level) + " is below " + str(ANDROID_API_LEVEL))
    elif request.api_level != 0 and request.api_level not in PLATFORM_ARTIFACTS:
        levels = ", ".join([str(level) for level in sorted(PLATFORM_ARTIFACTS.keys())])
        errors.append("android: api_level " + str(request.api_level) + " is not pinned; want one of " + levels)
    seen = []
    for package in request.extra_packages:
        if package not in OPT_IN_PACKAGES:
            errors.append("android: extra package '" + package + "' must be one of " + ", ".join(OPT_IN_PACKAGES))
        elif package in seen:
            errors.append("android: extra package '" + package + "' is listed twice")
        seen.append(package)
    errors.extend(license_errors(request))
    if request.mirror != "" and not request.mirror.startswith("https://"):
        errors.append("android: mirror '" + request.mirror + "' must use https")
    return errors

def sdk_level(request):
    """Returns the platform API level one request resolves to."""
    return request.api_level if request.api_level != 0 else DEFAULT_PLATFORM_LEVEL

def app_sdk_layout(request, host):
    """Returns the SDK directories rules_android's @androidsdk is assembled from on one host."""
    if not request.enabled or host not in SDK_HOSTS:
        return []
    level = sdk_level(request)
    host = SDK_HOSTS[host]
    return [
        struct(dest = "build-tools/" + BUILD_TOOLS, kind = "build-tools", artifact = BUILD_TOOLS_ARTIFACTS[host]),
        struct(dest = "platforms/android-%d" % level, kind = "platform", artifact = PLATFORM_ARTIFACTS[level]),
        struct(dest = "platform-tools", kind = "platform-tools", artifact = PLATFORM_TOOLS_ARTIFACTS[host]),
    ]

def app_sdk_substitutions(request = None):
    """Returns the template keys rules_android's SDK BUILD template takes."""
    level = str(sdk_level(request) if request else DEFAULT_PLATFORM_LEVEL)
    return {
        "__api_levels__": '"' + level + '"',
        "__build_tools_directory__": BUILD_TOOLS,
        "__build_tools_version__": BUILD_TOOLS,
        "__default_api_level__": level,
        "__system_image_dirs__": "",
    }

def app_sdk_hub_build(files_repo, level):
    """Returns the @androidsdk BUILD file that registers toolchains without fetching files_repo."""
    lines = [
        'load("@bazel_skylib//rules:common_settings.bzl", "string_flag")',
        'load("@platforms//host:constraints.bzl", "HOST_CONSTRAINTS")',
        "",
        'package(default_visibility = ["//visibility:public"])',
        "",
        'toolchain_type(name = "sdk_toolchain_type")',
        "",
        "config_feature_flag(",
        '    name = "true",',
        '    allowed_values = ["true", "false"],',
        '    default_value = "true",',
        ")",
        "",
        "config_setting(",
        '    name = "always_true",',
        '    flag_values = {":true": "true"},',
        ")",
        "",
        "config_setting(",
        '    name = "always_false",',
        '    flag_values = {":true": "false"},',
        ")",
        "",
        "alias(",
        '    name = "has_androidsdk",',
        '    actual = ":always_true",',
        ")",
        "",
        "string_flag(",
        '    name = "api_level",',
        '    build_setting_default = "%d",' % level,
        '    values = ["%d"],' % level,
        ")",
        "",
        "config_setting(",
        '    name = "api_%d_enabled",' % level,
        '    flag_values = {":api_level": "%d"},' % level,
        ")",
        "",
        "toolchain(",
        '    name = "sdk-%d-toolchain",' % level,
        "    exec_compatible_with = HOST_CONSTRAINTS,",
        '    target_settings = [":api_%d_enabled"],' % level,
        '    toolchain = "@%s//:sdk-%d",' % (files_repo, level),
        '    toolchain_type = ":sdk_toolchain_type",',
        ")",
        "",
        "toolchain(",
        '    name = "sdk-dummy-toolchain",',
        '    toolchain = "@%s//:sdk-dummy",' % files_repo,
        '    toolchain_type = ":sdk_toolchain_type",',
        ")",
        "",
        "alias(",
        '    name = "sdk-toolchain",',
        '    actual = ":sdk-%d-toolchain",' % level,
        ")",
    ]
    for name in APP_SDK_FORWARDED:
        target = name % level if "%d" in name else name
        lines.extend([
            "",
            "alias(",
            '    name = "%s",' % target,
            '    actual = "@%s//:%s",' % (files_repo, target),
            ")",
        ])
    return "\n".join(lines) + "\n"

def package_repositories(request):
    """Returns {kind: {host_or_level: repository}} for the packages one request fetches."""
    if not request.enabled:
        return {}
    level = sdk_level(request)
    repos = {
        "build_tools": build_tools_repositories(),
        "ndk": ndk_repositories(),
        "platform": {level: platform_repository_name()},
    }
    if "platform-tools" in request.extra_packages:
        repos["platform_tools"] = platform_tools_repositories()
    if "emulator" in request.extra_packages:
        repos["emulator"] = {host: emulator_repository_name(host) for host in sorted(EMULATOR_ARTIFACTS.keys())}
    return repos

def fetches_payload(requests):
    """Returns whether any request downloads an NDK archive."""
    for request in requests:
        if request.enabled:
            return True
    return False

def _hub_target(lines, name, repos, actual, error):
    """Appends one hub alias, or an empty filegroup when nothing is acquired."""
    if len(repos) > 0:
        lines.append("alias(")
        lines.append('    name = "%s",' % name)
        lines.append("    actual = select({")
        for platform in sorted(repos.keys()):
            lines.append('        ":%s": "@%s//:%s",' % (platform, repos[platform], actual))
        for platform in sorted(SDK_UNSUPPORTED_HOSTS.keys()):
            lines.append('        ":%s": ":no_sdk_host",' % platform)
        lines.append("    }, no_match_error = \"%s\")," % error)
        lines.append('    visibility = ["//visibility:public"],')
        lines.append(")")
    else:
        lines.append("filegroup(")
        lines.append('    name = "%s",' % name)
        lines.append("    srcs = [],")
        lines.append('    visibility = ["//visibility:public"],')
        lines.append(")")

def _hub_alias(lines, name, label):
    """Appends one plain hub alias, or an empty filegroup when there is no label."""
    if label != "":
        lines.append("alias(")
        lines.append('    name = "%s",' % name)
        lines.append('    actual = "%s",' % label)
        lines.append('    visibility = ["//visibility:public"],')
        lines.append(")")
    else:
        lines.append("filegroup(")
        lines.append('    name = "%s",' % name)
        lines.append("    srcs = [],")
        lines.append('    visibility = ["//visibility:public"],')
        lines.append(")")

def package_no_match_error(package, hosts):
    """Returns the diagnostic for a platform with no pinned opt-in package."""
    return "rules_dx: no pinned Android " + package + " artifact for this platform; want one of " + ", ".join(sorted(hosts))

def hub_build(ndk_repos, build_tools_repos, bundletool_repo, *, platform_repo = "", platform_tools_repos = {}, emulator_repos = {}):
    """Returns the hub BUILD file for one acquisition state."""
    if bundletool_repo == "":
        fail("rules_dx: hub_build needs the bundletool repository name")
    lines = []
    constraints = PLATFORM_CONSTRAINTS | SDK_UNSUPPORTED_HOSTS
    for platform in sorted(constraints.keys()):
        lines.append("config_setting(")
        lines.append('    name = "%s",' % platform)
        lines.append("    constraint_values = [")
        for constraint in constraints[platform]:
            lines.append('        "%s",' % constraint)
        lines.append("    ],")
        lines.append(")")
    lines.append("")
    lines.append("filegroup(")
    lines.append('    name = "no_sdk_host",')
    lines.append("    srcs = [],")
    lines.append("    target_compatible_with = [%s]," % ", ".join(['"%s"' % label for label in _NO_SDK_HOST_COMPATIBLE_WITH]))
    lines.append(")")
    lines.append("")
    _hub_target(lines, "ndk", ndk_repos, "ndk_anchor", no_match_error())
    lines.append("")
    _hub_target(lines, "zipalign", build_tools_repos, "zipalign", build_tools_no_match_error())
    lines.append("")
    _hub_target(lines, "aapt", build_tools_repos, "aapt", build_tools_no_match_error())
    lines.append("")
    _hub_target(lines, "dexdump", build_tools_repos, "dexdump", build_tools_no_match_error())
    lines.append("")
    _hub_target(lines, "apksigner_jar", build_tools_repos, "apksigner_jar", build_tools_no_match_error())
    lines.append("")
    _hub_alias(lines, "bundletool_jar", "@%s//file" % bundletool_repo)
    lines.append("")
    _hub_alias(lines, "android_jar", "@%s//:android_jar" % platform_repo if platform_repo != "" else "")
    lines.append("")
    _hub_target(lines, "adb", platform_tools_repos, "adb", package_no_match_error("platform-tools", SDK_HOSTS.keys()))
    lines.append("")
    _hub_target(lines, "emulator", emulator_repos, "emulator", package_no_match_error("emulator", EMULATOR_ARTIFACTS.keys()))
    return "\n".join(lines) + "\n"

NDK_RUNTIME_LIBS = {
    "linux_x86_64": ["lib/libxml2.so.2"],
    "macos_arm64": ["lib/libc++.dylib", "lib/libxml2.2.dylib"],
    "macos_x86_64": ["lib/libc++.dylib", "lib/libxml2.2.dylib"],
    "windows_arm64": ["bin/libwinpthread-1.dll", "bin/libxml2.dll"],
    "windows_x86_64": ["bin/libwinpthread-1.dll", "bin/libxml2.dll"],
}

def ndk_toolchain_build(root, host):
    """Returns the filegroups a cc toolchain needs from one host's NDK unpacked under root."""
    prebuilt = root + "/" + NDK_PREBUILT[host]
    suffix = NDK_TOOL_SUFFIX[host]
    lines = []
    for name, tool in [("clang", "clang"), ("ld_lld", "ld.lld"), ("llvm_ar", "llvm-ar"), ("llvm_readelf", "llvm-readelf"), ("llvm_strip", "llvm-strip")]:
        lines.extend([
            "filegroup(",
            '    name = "%s",' % name,
            '    srcs = ["%s/bin/%s%s"],' % (prebuilt, tool, suffix),
            '    visibility = ["//visibility:public"],',
            ")",
            "",
        ])
    lines.extend([
        "filegroup(",
        '    name = "runtime_libs",',
        "    srcs = [%s]," % ", ".join(['"%s/%s"' % (prebuilt, lib) for lib in NDK_RUNTIME_LIBS[host]]),
        '    visibility = ["//visibility:public"],',
        ")",
        "",
    ])
    lines.extend([
        "filegroup(",
        '    name = "compiler_files",',
        "    srcs = glob([",
        '        "%s/lib/clang/%s/include/**",' % (prebuilt, ANDROID_NDK_CLANG_VERSION),
        '        "%s/sysroot/usr/include/**",' % prebuilt,
        "    ]),",
        '    visibility = ["//visibility:public"],',
        ")",
        "",
    ])
    for triple in NDK_TRIPLES:
        lines.extend([
            "filegroup(",
            '    name = "linker_files_%s",' % triple,
            "    srcs = glob([",
            '        "%s/lib/clang/%s/lib/linux/**",' % (prebuilt, ANDROID_NDK_CLANG_VERSION),
            '        "%s/sysroot/usr/lib/%s/**",' % (prebuilt, triple),
            "    ]),",
            '    visibility = ["//visibility:public"],',
            ")",
            "",
        ])
    return "\n".join(lines)

def ndk_build(anchor, host):
    """Returns the BUILD file exposing one host's acquired NDK anchor."""
    return "\n".join([
        "filegroup(",
        '    name = "ndk",',
        '    srcs = glob(["extracted/**"]),',
        '    visibility = ["//visibility:public"],',
        ")",
        "",
        "filegroup(",
        '    name = "ndk_anchor",',
        '    srcs = ["extracted/%s"],' % anchor,
        '    visibility = ["//visibility:public"],',
        ")",
        "",
        ndk_toolchain_build("extracted", host),
    ])

def _tool_lines(name, path, data):
    """Returns one filegroup exposing a tool and the files it loads."""
    lines = [
        "filegroup(",
        '    name = "%s",' % name,
        '    srcs = ["extracted/%s"],' % path,
    ]
    if len(data) > 0:
        lines.append("    data = [")
        lines.extend(['        "extracted/%s",' % lib for lib in data])
        lines.append("    ],")
    lines.extend([
        '    visibility = ["//visibility:public"],',
        ")",
        "",
    ])
    return lines

def build_tools_build(anchor, host):
    """Returns the BUILD file exposing one host's acquired build-tools archive."""
    lines = []
    for name in ["zipalign", "aapt", "dexdump"]:
        lines.extend(_tool_lines(name, name + NDK_TOOL_SUFFIX[host], BUILD_TOOLS_LIBS[host]))
    lines.extend(_tool_lines("apksigner_jar", "lib/apksigner.jar", []))
    lines.extend(_tool_lines("build_tools_anchor", anchor, []))
    return "\n".join(lines)

def platform_build(anchor):
    """Returns the BUILD file exposing one acquired platform."""
    return "\n".join([
        "filegroup(",
        '    name = "android_jar",',
        '    srcs = ["extracted/android.jar"],',
        '    visibility = ["//visibility:public"],',
        ")",
        "",
        "filegroup(",
        '    name = "platform_anchor",',
        '    srcs = ["extracted/%s"],' % anchor,
        '    visibility = ["//visibility:public"],',
        ")",
        "",
    ])

def platform_tools_build(anchor, host):
    """Returns the BUILD file exposing one host's acquired platform-tools archive."""
    lines = _tool_lines("adb", "adb" + NDK_TOOL_SUFFIX[host], PLATFORM_TOOLS_LIBS[host])
    lines.extend(_tool_lines("platform_tools_anchor", anchor, []))
    return "\n".join(lines)

def emulator_build(anchor):
    """Returns the BUILD file exposing one acquired emulator archive."""
    return "\n".join([
        "filegroup(",
        '    name = "emulator",',
        '    srcs = ["extracted/emulator"],',
        '    data = glob(["extracted/**"]),',
        '    visibility = ["//visibility:public"],',
        ")",
        "",
        "filegroup(",
        '    name = "emulator_anchor",',
        '    srcs = ["extracted/%s"],' % anchor,
        '    visibility = ["//visibility:public"],',
        ")",
        "",
    ])

def _acquire(ctx, errors, build):
    """Downloads, verifies, and extracts one pinned archive."""
    if ctx.attr.license_error != "":
        fail(ctx.attr.license_error)
    if len(errors) > 0:
        fail("android: " + "; ".join(errors))
    ctx.download_and_extract(
        url = ctx.attr.url,
        sha256 = ctx.attr.sha256,
        canonical_id = ctx.attr.canonical_id,
        output = "extracted",
        strip_prefix = ctx.attr.directory,
    )
    if not ctx.path("extracted/" + ctx.attr.anchor).exists:
        fail("android: '" + ctx.attr.archive + "' holds no '" + ctx.attr.anchor + "'")
    ctx.file("BUILD.bazel", build(ctx.attr.anchor))
    return ctx.repo_metadata(reproducible = True)

def acquire_ndk(ctx):
    """Downloads, verifies, and extracts one pinned NDK archive."""
    return _acquire(ctx, artifact_errors(), lambda anchor: ndk_build(anchor, ctx.attr.host))

def acquire_build_tools(ctx):
    """Downloads, verifies, and extracts one pinned build-tools archive."""
    return _acquire(ctx, artifact_errors(BUILD_TOOLS_ARTIFACTS, BUILD_TOOLS_ARCHIVE_PREFIX, True, BUILD_TOOLS_ARCHIVE_SEPARATOR), lambda anchor: build_tools_build(anchor, ctx.attr.host))

def acquire_platform(ctx):
    """Downloads, verifies, and extracts one pinned platform archive."""
    return _acquire(ctx, artifact_errors(PLATFORM_ARTIFACTS, "", False), platform_build)

def acquire_platform_tools(ctx):
    """Downloads, verifies, and extracts one pinned platform-tools archive."""
    return _acquire(ctx, artifact_errors(PLATFORM_TOOLS_ARTIFACTS, PLATFORM_TOOLS_ARCHIVE_PREFIX), lambda anchor: platform_tools_build(anchor, ctx.attr.host))

def acquire_emulator(ctx):
    """Downloads, verifies, and extracts one pinned emulator archive."""
    return _acquire(ctx, artifact_errors(EMULATOR_ARTIFACTS, ""), emulator_build)

NDK_REPOSITORY_ATTRS = {
    "anchor": attr.string(
        doc = "Path inside the extracted archive that must exist.",
        mandatory = True,
    ),
    "archive": attr.string(
        doc = "Archive file name, reported when the anchor is missing.",
        mandatory = True,
    ),
    "canonical_id": attr.string(
        doc = "Stable cache identity for the downloaded archive.",
        mandatory = True,
    ),
    "directory": attr.string(
        doc = "Top-level directory stripped when extracting the archive.",
        mandatory = True,
    ),
    "license_error": attr.string(doc = "Failure raised when the archive is fetched without its license accepted."),
    "sha256": attr.string(doc = "Expected SHA-256 of the downloaded archive.", mandatory = True),
    "url": attr.string(doc = "Download URL of the archive.", mandatory = True),
}

android_ndk_repo = repository_rule(
    implementation = acquire_ndk,
    attrs = NDK_REPOSITORY_ATTRS | {
        "host": attr.string(
            doc = "Execution platform the NDK archive is built for.",
            mandatory = True,
            values = sorted(NDK_ARTIFACTS.keys()),
        ),
    },
)

android_build_tools_repo = repository_rule(
    implementation = acquire_build_tools,
    attrs = NDK_REPOSITORY_ATTRS | {
        "host": attr.string(
            doc = "Host the build-tools archive is built for.",
            mandatory = True,
            values = sorted(BUILD_TOOLS_ARTIFACTS.keys()),
        ),
    },
)

android_platform_repo = repository_rule(
    implementation = acquire_platform,
    attrs = NDK_REPOSITORY_ATTRS,
)

android_platform_tools_repo = repository_rule(
    implementation = acquire_platform_tools,
    attrs = NDK_REPOSITORY_ATTRS | {
        "host": attr.string(
            doc = "Host the platform-tools archive is built for.",
            mandatory = True,
            values = sorted(PLATFORM_TOOLS_ARTIFACTS.keys()),
        ),
    },
)

android_emulator_repo = repository_rule(
    implementation = acquire_emulator,
    attrs = NDK_REPOSITORY_ATTRS,
)

def _android_app_sdk_repo_impl(ctx):
    dirs = json.decode(ctx.attr.host_dirs.get(host_cell(ctx), "{}"))
    if len(dirs) == 0:
        ctx.template("BUILD.bazel", Label("@rules_android//rules/android_sdk_repository:empty.template.bzl"))
        return ctx.repo_metadata(reproducible = True)
    if ctx.attr.license_error != "":
        fail(ctx.attr.license_error)
    for dest, fields in dirs.items():
        ctx.download_and_extract(
            url = fields[0],
            sha256 = fields[1],
            canonical_id = fields[2],
            output = dest,
            strip_prefix = fields[3],
        )
    ctx.symlink(Label("@rules_android//rules/android_sdk_repository:helper.bzl"), "helper.bzl")
    substitutions = dict(ctx.attr.substitutions)
    substitutions["__repository_name__"] = ctx.name
    ctx.template("BUILD.bazel", Label("@rules_android//rules/android_sdk_repository:template.bzl"), substitutions = substitutions)
    return ctx.repo_metadata(reproducible = True)

android_app_sdk_repo = repository_rule(
    implementation = _android_app_sdk_repo_impl,
    attrs = {
        "host_dirs": attr.string_dict(
            doc = "Host cell to a JSON map of directory under the SDK root to [url, sha256, canonical_id, strip_prefix].",
        ),
        "license_error": attr.string(doc = "Failure raised when the SDK is fetched without its license accepted."),
        "substitutions": attr.string_dict(doc = "Keys rules_android's SDK BUILD template takes."),
    },
)

_EMPTY_SDK_TARGET = """filegroup(
    name = "sdk",
    srcs = [":error_message"],
)"""

def unavailable_hub_build(empty_template, rule_bzl):
    """Returns rules_android's empty SDK BUILD with :sdk as a ToolchainInfo target and an error stub per missing forwarded name."""
    if empty_template.count(_EMPTY_SDK_TARGET) != 1:
        fail("android: rules_android's empty SDK template no longer declares :sdk as a filegroup")
    sdk = 'unavailable_android_sdk(name = "sdk")'
    build = 'load("%s", "unavailable_android_sdk")\n\n' % rule_bzl + empty_template.replace(_EMPTY_SDK_TARGET, sdk)

    # bazel query loads every target rules_android names, on every host.
    stubs = [
        'filegroup(\n    name = "%s",\n    srcs = [":error_message"],\n)\n' % name
        for name in APP_SDK_FORWARDED
        if "%d" not in name and 'name = "%s"' % name not in build
    ]
    return build + "".join(["\n" + stub for stub in stubs])

def _android_app_sdk_hub_repo_impl(ctx):
    if ctx.attr.build == "" or host_cell(ctx) not in SDK_HOSTS:
        empty = ctx.read(Label("@rules_android//rules/android_sdk_repository:empty.template.bzl"))
        ctx.file("BUILD.bazel", unavailable_hub_build(empty, str(Label("//android/private/sdk:unavailable.bzl"))))
    else:
        ctx.file("BUILD.bazel", ctx.attr.build)
    return ctx.repo_metadata(reproducible = True)

android_app_sdk_hub_repo = repository_rule(
    implementation = _android_app_sdk_hub_repo_impl,
    attrs = {
        "build": attr.string(doc = "BUILD file of the @androidsdk hub, empty when no SDK is configured."),
    },
)
