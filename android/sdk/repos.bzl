"""Opt-in Android SDK and NDK acquisition for native builds."""

ANDROID_NDK_REVISION = "27.0.12077973"

ANDROID_NDK_RELEASE = "r27"

ANDROID_API_LEVEL = 31

ANDROID_TARGET = "aarch64-linux-android"

SDK_PLATFORM = "android-34"

BUILD_TOOLS = "34.0.0"

CMDLINE_TOOLS = "13.0"

MIN_JDK_VERSION = 17

DEFAULT_MIRROR = "https://dl.google.com/android/repository"

REQUIRED_LICENSES = ["android-sdk-license"]

SDK_PACKAGES = [
    "platforms;android-34",
    "build-tools;34.0.0",
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
        size = 663957918,
    ),
}

PLATFORM_CONSTRAINTS = {
    "linux_x86_64": ["@platforms//os:linux", "@platforms//cpu:x86_64"],
}

HUB_NAME = "dx_android_sdk"

_NDK_REPOSITORY_PREFIX = "dx_android_ndk_"

_ARTIFACT_KEYS = [
    "anchor",
    "archive",
    "directory",
    "sha1",
    "size",
]

_VERIFY_SCRIPT = """import hashlib, os, sys
path, want_sha1, want_size = sys.argv[1], sys.argv[2], int(sys.argv[3])
digest = hashlib.sha1()
size = 0
with open(path, "rb") as handle:
    for chunk in iter(lambda: handle.read(1048576), b""):
        digest.update(chunk)
        size += len(chunk)
if size != want_size:
    sys.exit("size " + str(size) + " of " + path + ", want " + str(want_size))
if digest.hexdigest() != want_sha1:
    sys.exit("sha1 mismatch on " + path)
"""

def ndk_repository_name(platform):
    """Returns the repository name one platform's NDK is acquired into."""
    return _NDK_REPOSITORY_PREFIX + platform

def ndk_repositories():
    """Returns the platform-keyed NDK repository names."""
    return {platform: ndk_repository_name(platform) for platform in sorted(NDK_ARTIFACTS.keys())}

def no_match_error():
    """Returns the diagnostic for an execution platform with no qualified NDK."""
    return ("rules_dx: no pinned Android NDK " + ANDROID_NDK_RELEASE + " artifact for this execution " +
            "platform; want one of " + ", ".join(sorted(NDK_ARTIFACTS.keys())))

def artifact_errors():
    """Returns one error string per NDK artifact that is not ready to download."""
    errors = []
    for platform in sorted(NDK_ARTIFACTS.keys()):
        if platform not in PLATFORM_CONSTRAINTS:
            errors.append("artifact '" + platform + "' names no platform constraints")
        artifact = NDK_ARTIFACTS[platform]
        missing = [key for key in _ARTIFACT_KEYS if getattr(artifact, key, "") == ""]
        if len(missing) > 0:
            errors.append("artifact '" + platform + "' is missing " + ", ".join(missing))
            continue
        expected = "android-ndk-" + ANDROID_NDK_RELEASE + "-" + _release_suffix(platform)
        if artifact.archive != expected:
            errors.append("artifact '" + platform + "' archive is '" + artifact.archive +
                          "', want the pinned '" + expected + "'")
        if len(artifact.sha1) != 40:
            errors.append("artifact '" + platform + "' sha1 is not 40 hex characters")
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
    """Returns the download URL one pinned NDK artifact lives at."""
    base = mirror if mirror != "" else DEFAULT_MIRROR
    return base + "/" + artifact.archive

def canonical_id_for(url):
    """Returns the stable cache identity for one NDK download URL."""
    return "dx-android-ndk-archive:" + url

def default_request():
    """Returns the opt-in request enabling the qualified host NDK."""
    return struct(
        accept_licenses = list(REQUIRED_LICENSES),
        api_level = ANDROID_API_LEVEL,
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
    if request.local_path != "":
        return errors
    missing = [name for name in REQUIRED_LICENSES if name not in request.accept_licenses]
    if len(missing) > 0:
        errors.append("android_sdk: accept " + ", ".join(missing) + " before acquisition")
    if request.mirror != "" and not request.mirror.startswith("https://"):
        errors.append("android_sdk: mirror '" + request.mirror + "' must use https")
    if request.api_level != 0 and request.api_level < ANDROID_API_LEVEL:
        errors.append("android_sdk: api_level " + str(request.api_level) + " is below " + str(ANDROID_API_LEVEL))
    for package in request.extra_packages:
        if package == "" or package in SDK_PACKAGES:
            errors.append("android_sdk: extra package '" + package + "' is empty or already pinned")
    return errors

def fetches_payload(requests):
    """Returns whether any request downloads an NDK archive."""
    for request in requests:
        if request.enabled and request.local_path == "":
            return True
    return False

def hub_build(ndk_repos):
    """Returns the hub BUILD file for one acquisition state."""
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
    if len(ndk_repos) > 0:
        lines.append("alias(")
        lines.append('    name = "ndk",')
        lines.append("    actual = select({")
        for platform in sorted(ndk_repos.keys()):
            lines.append('        ":%s": "@%s//:ndk_anchor",' % (platform, ndk_repos[platform]))
        lines.append("    }, no_match_error = \"%s\")," % no_match_error())
        lines.append('    visibility = ["//visibility:public"],')
        lines.append(")")
    else:
        lines.append("filegroup(")
        lines.append('    name = "ndk",')
        lines.append("    srcs = [],")
        lines.append('    visibility = ["//visibility:public"],')
        lines.append(")")
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

def _verify_archive(ctx, path):
    python = ctx.which("python3")
    if python == None:
        fail("android_sdk: python3 is required to verify the NDK archive identity")
    result = ctx.execute([
        python,
        "-c",
        _VERIFY_SCRIPT,
        str(path),
        ctx.attr.sha1,
        str(ctx.attr.size),
    ])
    if result.return_code != 0:
        fail("android_sdk: '" + ctx.attr.archive + "' failed verification: " + result.stdout + result.stderr)

def _android_ndk_repo_impl(ctx):
    if len(artifact_errors()) > 0:
        fail("android_sdk: " + "; ".join(artifact_errors()))
    archive = ctx.download_and_extract(
        url = ctx.attr.url,
        output = "extracted",
        stripPrefix = ctx.attr.directory,
        canonical_id = ctx.attr.canonical_id,
    )
    _verify_archive(ctx, archive)
    if not ctx.path("extracted/" + ctx.attr.anchor).exists:
        fail("android_sdk: '" + ctx.attr.archive + "' holds no '" + ctx.attr.anchor + "'")
    ctx.file("BUILD.bazel", ndk_build(ctx.attr.anchor))

def _android_sdk_hub_repo_impl(ctx):
    ctx.file("BUILD.bazel", hub_build(json.decode(ctx.attr.ndk_repos)))

def _android_sdk_local_repo_impl(ctx):
    if not ctx.path(ctx.attr.local_path + "/licenses/android-sdk-license").exists:
        fail("android_sdk: '" + ctx.attr.local_path + "' shows no accepted android-sdk-license")
    anchor = ctx.attr.local_path + "/ndk/" + ANDROID_NDK_REVISION + "/source.properties"
    if not ctx.path(anchor).exists:
        fail("android_sdk: '" + ctx.attr.local_path + "' holds no pinned NDK " + ANDROID_NDK_REVISION)
    ctx.symlink(ctx.path(ctx.attr.local_path), "sdk")
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
    ]))

_android_ndk_repo = repository_rule(
    implementation = _android_ndk_repo_impl,
    attrs = {
        "anchor": attr.string(mandatory = True),
        "archive": attr.string(mandatory = True),
        "canonical_id": attr.string(mandatory = True),
        "directory": attr.string(mandatory = True),
        "sha1": attr.string(mandatory = True),
        "size": attr.int(mandatory = True),
        "url": attr.string(mandatory = True),
    },
)

_android_sdk_hub_repo = repository_rule(
    implementation = _android_sdk_hub_repo_impl,
    attrs = {
        "ndk_repos": attr.string(mandatory = True),
    },
)

_android_sdk_local_repo = repository_rule(
    implementation = _android_sdk_local_repo_impl,
    attrs = {
        "local_path": attr.string(mandatory = True),
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

def _android_sdk_impl(module_ctx):
    if len(module_ctx.modules) == 0:
        fail("android_sdk: the extension is only reachable from a module graph")
    errors = artifact_errors()
    if len(errors) > 0:
        fail("android_sdk: " + "; ".join(errors))
    requests = []
    for module in module_ctx.modules:
        for tag in module.tags.configure:
            requests.append(_configure_tag(tag))
    if len(requests) > 1:
        fail("android_sdk: configure the Android SDK once")
    if len(requests) == 0:
        _android_sdk_hub_repo(name = HUB_NAME, ndk_repos = "{}")
        return
    request = requests[0]
    failures = request_errors(request)
    if len(failures) > 0:
        fail("android_sdk: " + "; ".join(failures))
    if not request.enabled:
        _android_sdk_hub_repo(name = HUB_NAME, ndk_repos = "{}")
        return
    if request.local_path != "":
        local_repo = HUB_NAME + "_local"
        _android_sdk_local_repo(name = local_repo, local_path = request.local_path)
        _android_sdk_hub_repo(name = HUB_NAME, ndk_repos = json.encode({"linux_x86_64": local_repo}))
        return
    mirror = request.mirror if request.mirror != "" else DEFAULT_MIRROR
    for platform in sorted(NDK_ARTIFACTS.keys()):
        artifact = NDK_ARTIFACTS[platform]
        url = url_for(artifact, mirror)
        _android_ndk_repo(
            name = ndk_repository_name(platform),
            anchor = artifact.anchor,
            archive = artifact.archive,
            canonical_id = canonical_id_for(url),
            directory = artifact.directory,
            sha1 = artifact.sha1,
            size = artifact.size,
            url = url,
        )
    _android_sdk_hub_repo(name = HUB_NAME, ndk_repos = json.encode(ndk_repositories()))

android_sdk = module_extension(
    implementation = _android_sdk_impl,
    tag_classes = {
        "configure": tag_class(attrs = {
            "accept_licenses": attr.string_list(default = []),
            "api_level": attr.int(default = 0),
            "enabled": attr.bool(default = True),
            "extra_packages": attr.string_list(default = []),
            "local_path": attr.string(default = ""),
            "mirror": attr.string(default = ""),
        }),
    },
)
