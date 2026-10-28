"""Opt-in Android NDK and build-tools acquisition."""

load("@bazel_tools//tools/build_defs/repo:http.bzl", "http_file")

ANDROID_NDK_REVISION = "27.0.12077973"

ANDROID_NDK_RELEASE = "r27"

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

NDK_ARTIFACTS = {
    "linux_x86_64": struct(
        anchor = "source.properties",
        archive = "android-ndk-r27-linux.zip",
        directory = "android-ndk-r27",
        sha1 = "5e5cd517bdb98d7e0faf2c494a3041291e71bdcc",
        sha256 = "2f17eb8bcbfdc40201c0b36e9a70826fcd2524ab7a2a235e2c71186c302da1dc",
        size = 663957918,
    ),
}

BUILD_TOOLS_ARTIFACTS = {
    "linux_x86_64": struct(
        anchor = "source.properties",
        archive = "build-tools_r35_linux.zip",
        directory = "android-15",
        sha1 = "2cfaa0bbb2336e9ec18ed3ecea84fa2e2af607bc",
        sha256 = "bd3a4966912eb8b30ed0d00b0cda6b6543b949d5ffe00bea54c04c81e1561d88",
        size = 61958799,
    ),
}

PLATFORM_ARTIFACTS = {
    31: struct(
        anchor = "android.jar",
        archive = "platform-31_r01.zip",
        directory = "android-12",
        sha1 = "ca5bcaa565cb37e9d287051d6dd0e49a5426ec29",
        sha256 = "1d69fe1d7f9788d82ff3a374faf4f6ccc9d1d372aa84a86b5bcfb517523b0b3f",
        size = 56475526,
    ),
    32: struct(
        anchor = "android.jar",
        archive = "platform-32_r01.zip",
        directory = "android-12",
        sha1 = "afae86ed55d29733d50996ffed832f2d1bd75b9a",
        sha256 = "01d8da1c900e70fcf5da39767d5444e39928935b1a5927055ce749fc348ca7ae",
        size = 66108299,
    ),
    33: struct(
        anchor = "android.jar",
        archive = "platform-33-ext3_r03.zip",
        directory = "android-13",
        sha1 = "394bc86d8d3452aa4d419b67743025a6fb2cd9d0",
        sha256 = "b32b10f787867987f03ae8e101d217e053a9065b7136379fb353b388379aed1d",
        size = 67334237,
    ),
    34: struct(
        anchor = "android.jar",
        archive = "platform-34-ext7_r03.zip",
        directory = "android-34",
        sha1 = "1f2e9478d6a7601425ceaa553311dc43191f103d",
        sha256 = "16fdb74c55e59ae3ef52def135aec713508467bd56d7dabcd8c9be31fa8b20f3",
        size = 63180081,
    ),
}

PLATFORM_TOOLS_ARTIFACTS = {
    "linux_x86_64": struct(
        anchor = "adb",
        archive = "platform-tools_r37.0.1-linux.zip",
        directory = "platform-tools",
        sha1 = "477254aa5f903c15cf51001717bdf347fb6b53e0",
        sha256 = "d230f13842f60f782a8645f9c813f8f845bf36089ea7289f28c48f17979313f1",
        size = 9054187,
    ),
}

EMULATOR_ARTIFACTS = {
    "linux_x86_64": struct(
        anchor = "emulator",
        archive = "emulator-linux_x64-16428233.zip",
        directory = "emulator",
        sha1 = "cd7362ea55dfb86a418958138dc396e74165dd01",
        sha256 = "b08fc43d8608d2955f607f1b287beb525041e730086bdca3af152accac3af9c1",
        size = 349654171,
    ),
}

NDK_ARCHIVE_PREFIX = "android-ndk-" + ANDROID_NDK_RELEASE

BUILD_TOOLS_ARCHIVE_PREFIX = "build-tools_r" + BUILD_TOOLS.split(".")[0]

# Google joins build-tools archive names with "_" since r35.
BUILD_TOOLS_ARCHIVE_SEPARATOR = "_"

PLATFORM_TOOLS_ARCHIVE_PREFIX = "platform-tools_r" + PLATFORM_TOOLS

PLATFORM_CONSTRAINTS = {
    "linux_x86_64": ["@platforms//os:linux", "@platforms//cpu:x86_64"],
}

HUB_NAME = "dx_android_sdk"

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
    """Returns the platform-keyed build-tools repository names."""
    return {platform: build_tools_repository_name(platform) for platform in sorted(BUILD_TOOLS_ARTIFACTS.keys())}

def platform_repository_name():
    """Returns the repository name the requested platform is acquired into."""
    return _PLATFORM_REPOSITORY

def platform_tools_repository_name(host):
    """Returns the repository name one host's platform-tools are acquired into."""
    return _PLATFORM_TOOLS_REPOSITORY_PREFIX + host

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
            "platform; want one of " + ", ".join(sorted(BUILD_TOOLS_ARTIFACTS.keys())))

def artifact_errors(artifacts = NDK_ARTIFACTS, prefix = NDK_ARCHIVE_PREFIX, hosted = True, separator = "-"):
    """Returns one error string per pinned artifact that is not ready to download.

    `separator` joins the archive prefix and the platform suffix; build-tools
    archives use "_" (build-tools_r35_linux.zip), the others use "-".
    """
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
            expected = prefix + separator + _release_suffix(platform)
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

def _release_suffix(platform):
    """Returns the upstream file suffix for one qualified execution platform."""
    parts = platform.split("_", 1)
    if parts[0] == "linux":
        return "linux.zip"
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
        local_path = "",
        mirror = "",
    )

def request_errors(request):
    """Returns one error string per rejected field of one acquisition request."""
    errors = []
    if not request.enabled:
        return errors
    if request.api_level != 0 and request.api_level < ANDROID_API_LEVEL:
        errors.append("android_sdk: api_level " + str(request.api_level) + " is below " + str(ANDROID_API_LEVEL))
    elif request.api_level != 0 and request.api_level not in PLATFORM_ARTIFACTS:
        levels = ", ".join([str(level) for level in sorted(PLATFORM_ARTIFACTS.keys())])
        errors.append("android_sdk: api_level " + str(request.api_level) + " is not pinned; want one of " + levels)
    seen = []
    for package in request.extra_packages:
        if package not in OPT_IN_PACKAGES:
            errors.append("android_sdk: extra package '" + package + "' must be one of " + ", ".join(OPT_IN_PACKAGES))
        elif package in seen:
            errors.append("android_sdk: extra package '" + package + "' is listed twice")
        seen.append(package)
    if request.local_path != "":
        return errors
    missing = [name for name in REQUIRED_LICENSES if name not in request.accept_licenses]
    if len(missing) > 0:
        errors.append("android_sdk: accept " + ", ".join(missing) + " before acquisition")
    if request.mirror != "" and not request.mirror.startswith("https://"):
        errors.append("android_sdk: mirror '" + request.mirror + "' must use https")
    return errors

def _level(request):
    """Returns the platform API level one request resolves to."""
    return request.api_level if request.api_level != 0 else DEFAULT_PLATFORM_LEVEL

def package_repositories(request):
    """Returns {kind: {host_or_level: repository}} for the packages one request fetches."""
    if not request.enabled:
        return {}
    level = _level(request)
    if request.local_path != "":
        local = HUB_NAME + "_local"
        repos = {
            "build_tools": {"linux_x86_64": local},
            "ndk": {"linux_x86_64": local},
            "platform": {level: local},
        }
        if "platform-tools" in request.extra_packages:
            repos["platform_tools"] = {"linux_x86_64": local}
        if "emulator" in request.extra_packages:
            repos["emulator"] = {"linux_x86_64": local}
        return repos
    repos = {
        "build_tools": build_tools_repositories(),
        "ndk": ndk_repositories(),
        "platform": {level: platform_repository_name()},
    }
    if "platform-tools" in request.extra_packages:
        repos["platform_tools"] = {host: platform_tools_repository_name(host) for host in sorted(PLATFORM_TOOLS_ARTIFACTS.keys())}
    if "emulator" in request.extra_packages:
        repos["emulator"] = {host: emulator_repository_name(host) for host in sorted(EMULATOR_ARTIFACTS.keys())}
    return repos

def fetches_payload(requests):
    """Returns whether any request downloads an NDK archive."""
    for request in requests:
        if request.enabled and request.local_path == "":
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

def package_no_match_error(package):
    """Returns the diagnostic for a platform with no pinned opt-in package."""
    return "rules_dx: no pinned Android " + package + " artifact for this platform; want one of linux_x86_64"

def hub_build(ndk_repos, build_tools_repos, bundletool_repo, *, platform_repo = "", platform_tools_repos = {}, emulator_repos = {}):
    """Returns the hub BUILD file for one acquisition state."""
    if bundletool_repo == "":
        fail("rules_dx: hub_build needs the bundletool repository name")
    lines = []
    for platform in sorted(PLATFORM_CONSTRAINTS.keys()):
        lines.append("config_setting(")
        lines.append('    name = "%s",' % platform)
        lines.append("    constraint_values = [")
        for constraint in PLATFORM_CONSTRAINTS[platform]:
            lines.append('        "%s",' % constraint)
        lines.append("    ],")
        lines.append(")")
    lines.append("")
    _hub_target(lines, "ndk", ndk_repos, "ndk_anchor", no_match_error())
    lines.append("")
    _hub_target(lines, "zipalign", build_tools_repos, "zipalign", build_tools_no_match_error())
    lines.append("")
    _hub_target(lines, "aapt", build_tools_repos, "aapt", build_tools_no_match_error())
    lines.append("")
    _hub_target(lines, "apksigner_jar", build_tools_repos, "apksigner_jar", build_tools_no_match_error())
    lines.append("")
    _hub_alias(lines, "bundletool_jar", "@%s//file" % bundletool_repo)
    lines.append("")
    _hub_alias(lines, "android_jar", "@%s//:android_jar" % platform_repo if platform_repo != "" else "")
    lines.append("")
    _hub_target(lines, "adb", platform_tools_repos, "adb", package_no_match_error("platform-tools"))
    lines.append("")
    _hub_target(lines, "emulator", emulator_repos, "emulator", package_no_match_error("emulator"))
    return "\n".join(lines) + "\n"

def ndk_build(anchor):
    """Returns the BUILD file exposing one acquired NDK anchor."""
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
    ])

def build_tools_build(anchor):
    """Returns the BUILD file exposing one acquired build-tools archive."""
    return "\n".join([
        "filegroup(",
        '    name = "zipalign",',
        '    srcs = ["extracted/zipalign"],',
        "    data = [",
        '        "extracted/lib64/libc++.so",',
        '        "extracted/lib64/libc++.so.1",',
        "    ],",
        '    visibility = ["//visibility:public"],',
        ")",
        "",
        "filegroup(",
        '    name = "aapt",',
        '    srcs = ["extracted/aapt"],',
        "    data = [",
        '        "extracted/lib64/libc++.so",',
        '        "extracted/lib64/libc++.so.1",',
        "    ],",
        '    visibility = ["//visibility:public"],',
        ")",
        "",
        "filegroup(",
        '    name = "apksigner_jar",',
        '    srcs = ["extracted/lib/apksigner.jar"],',
        '    visibility = ["//visibility:public"],',
        ")",
        "",
        "filegroup(",
        '    name = "build_tools_anchor",',
        '    srcs = ["extracted/%s"],' % anchor,
        '    visibility = ["//visibility:public"],',
        ")",
        "",
    ])

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

def platform_tools_build(anchor):
    """Returns the BUILD file exposing one acquired platform-tools archive."""
    return "\n".join([
        "filegroup(",
        '    name = "adb",',
        '    srcs = ["extracted/adb"],',
        '    visibility = ["//visibility:public"],',
        ")",
        "",
        "filegroup(",
        '    name = "platform_tools_anchor",',
        '    srcs = ["extracted/%s"],' % anchor,
        '    visibility = ["//visibility:public"],',
        ")",
        "",
    ])

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
    if len(errors) > 0:
        fail("android_sdk: " + "; ".join(errors))
    ctx.download_and_extract(
        url = ctx.attr.url,
        sha256 = ctx.attr.sha256,
        canonical_id = ctx.attr.canonical_id,
        output = "extracted",
        strip_prefix = ctx.attr.directory,
    )
    if not ctx.path("extracted/" + ctx.attr.anchor).exists:
        fail("android_sdk: '" + ctx.attr.archive + "' holds no '" + ctx.attr.anchor + "'")
    ctx.file("BUILD.bazel", build(ctx.attr.anchor))
    return ctx.repo_metadata(reproducible = True)

def acquire_ndk(ctx):
    """Downloads, verifies, and extracts one pinned NDK archive."""
    return _acquire(ctx, artifact_errors(), ndk_build)

def acquire_build_tools(ctx):
    """Downloads, verifies, and extracts one pinned build-tools archive."""
    return _acquire(ctx, artifact_errors(BUILD_TOOLS_ARTIFACTS, BUILD_TOOLS_ARCHIVE_PREFIX, True, BUILD_TOOLS_ARCHIVE_SEPARATOR), build_tools_build)

def acquire_platform(ctx):
    """Downloads, verifies, and extracts one pinned platform archive."""
    return _acquire(ctx, artifact_errors(PLATFORM_ARTIFACTS, "", False), platform_build)

def acquire_platform_tools(ctx):
    """Downloads, verifies, and extracts one pinned platform-tools archive."""
    return _acquire(ctx, artifact_errors(PLATFORM_TOOLS_ARTIFACTS, PLATFORM_TOOLS_ARCHIVE_PREFIX), platform_tools_build)

def acquire_emulator(ctx):
    """Downloads, verifies, and extracts one pinned emulator archive."""
    return _acquire(ctx, artifact_errors(EMULATOR_ARTIFACTS, ""), emulator_build)

def _android_sdk_hub_repo_impl(ctx):
    ctx.file("BUILD.bazel", hub_build(
        json.decode(ctx.attr.ndk_repos),
        json.decode(ctx.attr.build_tools_repos),
        ctx.attr.bundletool_repo,
        platform_repo = ctx.attr.platform_repo,
        platform_tools_repos = json.decode(ctx.attr.platform_tools_repos),
        emulator_repos = json.decode(ctx.attr.emulator_repos),
    ))
    return ctx.repo_metadata(reproducible = True)

def _require_local(ctx, package, path, required):
    """Fails naming the missing path when a requested package is absent from a local SDK."""
    full = ctx.attr.local_path + "/" + path
    if required and not ctx.path(full).exists:
        fail("android_sdk: '" + ctx.attr.local_path + "' holds no " + package + " at '" + full + "'")

def _android_sdk_local_repo_impl(ctx):
    if not ctx.path(ctx.attr.local_path + "/licenses/android-sdk-license").exists:
        fail("android_sdk: '" + ctx.attr.local_path + "' shows no accepted android-sdk-license")
    anchor = ctx.attr.local_path + "/ndk/" + ANDROID_NDK_REVISION + "/source.properties"
    if not ctx.path(anchor).exists:
        fail("android_sdk: '" + ctx.attr.local_path + "' holds no pinned NDK " + ANDROID_NDK_REVISION)
    level = ctx.attr.api_level if ctx.attr.api_level != 0 else DEFAULT_PLATFORM_LEVEL
    jar = "platforms/android-" + str(level) + "/android.jar"
    _require_local(ctx, "platform android-" + str(level), jar, ctx.attr.api_level != 0)
    _require_local(ctx, "platform-tools", "platform-tools/adb", "platform-tools" in ctx.attr.extra_packages)
    _require_local(ctx, "emulator", "emulator/emulator", "emulator" in ctx.attr.extra_packages)
    ctx.symlink(ctx.path(ctx.attr.local_path), "sdk")
    tools = "sdk/build-tools/" + BUILD_TOOLS + "/"
    ctx.file("BUILD.bazel", "\n".join([
        "filegroup(",
        '    name = "ndk",',
        '    srcs = glob(["sdk/ndk/' + ANDROID_NDK_REVISION + '/**"]),',
        '    visibility = ["//visibility:public"],',
        ")",
        "",
        "filegroup(",
        '    name = "ndk_anchor",',
        '    srcs = ["sdk/ndk/' + ANDROID_NDK_REVISION + '/source.properties"],',
        '    visibility = ["//visibility:public"],',
        ")",
        "",
        "filegroup(",
        '    name = "zipalign",',
        '    srcs = glob(["' + tools + 'zipalign"], allow_empty = True),',
        '    data = glob(["' + tools + 'lib64/libc++.so*"], allow_empty = True),',
        '    visibility = ["//visibility:public"],',
        ")",
        "",
        "filegroup(",
        '    name = "apksigner_jar",',
        '    srcs = glob(["' + tools + 'lib/apksigner.jar"], allow_empty = True),',
        '    visibility = ["//visibility:public"],',
        ")",
        "",
        "filegroup(",
        '    name = "build_tools_anchor",',
        '    srcs = glob(["' + tools + 'source.properties"], allow_empty = True),',
        '    visibility = ["//visibility:public"],',
        ")",
        "",
        "filegroup(",
        '    name = "android_jar",',
        '    srcs = glob(["sdk/' + jar + '"], allow_empty = True),',
        '    visibility = ["//visibility:public"],',
        ")",
        "",
        "filegroup(",
        '    name = "adb",',
        '    srcs = glob(["sdk/platform-tools/adb"], allow_empty = True),',
        '    visibility = ["//visibility:public"],',
        ")",
        "",
        "filegroup(",
        '    name = "emulator",',
        '    srcs = glob(["sdk/emulator/emulator"], allow_empty = True),',
        '    data = glob(["sdk/emulator/**"], allow_empty = True),',
        '    visibility = ["//visibility:public"],',
        ")",
        "",
    ]))

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
    "sha256": attr.string(doc = "Expected SHA-256 of the downloaded archive.", mandatory = True),
    "url": attr.string(doc = "Download URL of the archive.", mandatory = True),
}

_android_ndk_repo = repository_rule(
    implementation = acquire_ndk,
    attrs = NDK_REPOSITORY_ATTRS,
)

_android_build_tools_repo = repository_rule(
    implementation = acquire_build_tools,
    attrs = NDK_REPOSITORY_ATTRS,
)

_android_platform_repo = repository_rule(
    implementation = acquire_platform,
    attrs = NDK_REPOSITORY_ATTRS,
)

_android_platform_tools_repo = repository_rule(
    implementation = acquire_platform_tools,
    attrs = NDK_REPOSITORY_ATTRS,
)

_android_emulator_repo = repository_rule(
    implementation = acquire_emulator,
    attrs = NDK_REPOSITORY_ATTRS,
)

_android_sdk_hub_repo = repository_rule(
    implementation = _android_sdk_hub_repo_impl,
    attrs = {
        "build_tools_repos": attr.string(
            doc = "JSON map of build-tools repository names by platform.",
            mandatory = True,
        ),
        "bundletool_repo": attr.string(
            doc = "Repository name that provides the bundletool jar.",
            mandatory = True,
        ),
        "emulator_repos": attr.string(
            doc = "JSON map of emulator repository names by host.",
            default = "{}",
        ),
        "ndk_repos": attr.string(
            doc = "JSON map of NDK repository names by platform.",
            mandatory = True,
        ),
        "platform_repo": attr.string(
            doc = "Repository name of the default platform package.",
            default = "",
        ),
        "platform_tools_repos": attr.string(
            doc = "JSON map of platform-tools repository names by host.",
            default = "{}",
        ),
    },
)

_android_sdk_local_repo = repository_rule(
    implementation = _android_sdk_local_repo_impl,
    attrs = {
        "api_level": attr.int(
            doc = "Platform API level required, zero uses the default level.",
            default = 0,
        ),
        "extra_packages": attr.string_list(
            doc = "Extra SDK packages required locally, such as platform-tools or emulator.",
            default = [],
        ),
        "local_path": attr.string(
            doc = "Existing Android SDK directory used instead of downloads.",
            mandatory = True,
        ),
    },
)

def _configure_tag(request):
    return struct(
        accept_licenses = list(request.accept_licenses),
        api_level = request.api_level,
        enabled = request.enabled,
        extra_packages = list(request.extra_packages),
        local_path = request.local_path,
        mirror = request.mirror,
    )

def _declare(rule, name, artifact, kind, mirror):
    """Declares one pinned archive repository."""
    url = url_for(artifact, mirror)
    rule(
        name = name,
        anchor = artifact.anchor,
        archive = artifact.archive,
        canonical_id = canonical_id_for(url, kind),
        directory = artifact.directory,
        sha256 = artifact.sha256,
        url = url,
    )

def _android_sdk_impl(module_ctx):
    if len(module_ctx.modules) == 0:
        fail("android_sdk: the extension is only reachable from a module graph")
    errors = (
        artifact_errors() +
        artifact_errors(BUILD_TOOLS_ARTIFACTS, BUILD_TOOLS_ARCHIVE_PREFIX, True, BUILD_TOOLS_ARCHIVE_SEPARATOR) +
        artifact_errors(PLATFORM_ARTIFACTS, "", False) +
        artifact_errors(PLATFORM_TOOLS_ARTIFACTS, PLATFORM_TOOLS_ARCHIVE_PREFIX) +
        artifact_errors(EMULATOR_ARTIFACTS, "")
    )
    if len(errors) > 0:
        fail("android_sdk: " + "; ".join(errors))
    http_file(
        name = BUNDLETOOL_REPOSITORY,
        canonical_id = canonical_id_for(bundletool_url(), "bundletool"),
        downloaded_file_path = "bundletool-all.jar",
        sha256 = BUNDLETOOL_SHA256,
        urls = [bundletool_url()],
    )
    requests = []
    for module in module_ctx.modules:
        for tag in module.tags.configure:
            requests.append(_configure_tag(tag))
    if len(requests) > 1:
        fail("android_sdk: configure the Android SDK once")
    repos = {}
    if len(requests) == 1:
        request = requests[0]
        failures = request_errors(request)
        if len(failures) > 0:
            fail("android_sdk: " + "; ".join(failures))
        repos = package_repositories(request)
        if request.local_path != "":
            _android_sdk_local_repo(
                name = HUB_NAME + "_local",
                api_level = request.api_level,
                extra_packages = request.extra_packages,
                local_path = request.local_path,
            )
        elif request.enabled:
            mirror = request.mirror if request.mirror != "" else DEFAULT_MIRROR
            for platform in sorted(repos["ndk"].keys()):
                _declare(_android_ndk_repo, repos["ndk"][platform], NDK_ARTIFACTS[platform], "ndk", mirror)
            for platform in sorted(repos["build_tools"].keys()):
                _declare(_android_build_tools_repo, repos["build_tools"][platform], BUILD_TOOLS_ARTIFACTS[platform], "build-tools", mirror)
            for level in sorted(repos["platform"].keys()):
                _declare(_android_platform_repo, repos["platform"][level], PLATFORM_ARTIFACTS[level], "platform", mirror)
            for host in sorted(repos.get("platform_tools", {}).keys()):
                _declare(_android_platform_tools_repo, repos["platform_tools"][host], PLATFORM_TOOLS_ARTIFACTS[host], "platform-tools", mirror)
            for host in sorted(repos.get("emulator", {}).keys()):
                _declare(_android_emulator_repo, repos["emulator"][host], EMULATOR_ARTIFACTS[host], "emulator", mirror)
    platform_repos = repos.get("platform", {})
    _android_sdk_hub_repo(
        name = HUB_NAME,
        build_tools_repos = json.encode(repos.get("build_tools", {})),
        bundletool_repo = BUNDLETOOL_REPOSITORY,
        emulator_repos = json.encode(repos.get("emulator", {})),
        ndk_repos = json.encode(repos.get("ndk", {})),
        platform_repo = platform_repos[sorted(platform_repos.keys())[0]] if len(platform_repos) > 0 else "",
        platform_tools_repos = json.encode(repos.get("platform_tools", {})),
    )

android_sdk = module_extension(
    implementation = _android_sdk_impl,
    tag_classes = {
        "configure": tag_class(attrs = {
            "accept_licenses": attr.string_list(
                doc = "License identifiers the Android SDK configuration accepts.",
                default = [],
            ),
            "api_level": attr.int(
                doc = "Platform API level to configure, zero for the default.",
                default = 0,
            ),
            "enabled": attr.bool(
                doc = "Whether to download the configured SDK packages.",
                default = True,
            ),
            "extra_packages": attr.string_list(
                doc = "Extra SDK packages to fetch, such as platform-tools or emulator.",
                default = [],
            ),
            "local_path": attr.string(
                doc = "Existing Android SDK directory used instead of downloading.",
                default = "",
            ),
            "mirror": attr.string(
                doc = "Https base URL that SDK archives are downloaded from.",
                default = "",
            ),
        }),
    },
)
