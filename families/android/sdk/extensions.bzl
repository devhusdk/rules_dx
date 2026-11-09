"""The android_sdk module extension."""

load("@bazel_tools//tools/build_defs/repo:http.bzl", "http_file")
load("@rules_dx//libs/starlark:license.bzl", "license_tag", "root_accepts")
load("@rules_dx//libs/starlark:repo.bzl", "hub_repo")
load(":repos.bzl", "APP_SDK_FILES_NAME", "APP_SDK_HUB_NAME", "BUILD_TOOLS_ARCHIVE_PREFIX", "BUILD_TOOLS_ARCHIVE_SEPARATOR", "BUILD_TOOLS_ARTIFACTS", "BUNDLETOOL_REPOSITORY", "BUNDLETOOL_SHA256", "DEFAULT_MIRROR", "EMULATOR_ARTIFACTS", "HUB_NAME", "NDK_ARTIFACTS", "PLATFORM_ARTIFACTS", "PLATFORM_TOOLS_ARCHIVE_PREFIX", "PLATFORM_TOOLS_ARTIFACTS", "android_app_sdk_hub_repo", "android_app_sdk_repo", "android_build_tools_repo", "android_emulator_repo", "android_ndk_repo", "android_platform_repo", "android_platform_tools_repo", "android_sdk_local_repo", "app_sdk_hub_build", "app_sdk_layout", "app_sdk_substitutions", "artifact_errors", "bundletool_url", "canonical_id_for", "hub_build", "package_repositories", "request_errors", "sdk_level", "url_for")

def _configure_tag(request, accept_licenses):
    return struct(
        accept_licenses = accept_licenses,
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

def _declare_app_sdk(request):
    """Declares the @androidsdk replacement rules_android's SDK template builds on."""
    dirs = {}
    mirror = request.mirror if request.mirror != "" else DEFAULT_MIRROR
    for entry in app_sdk_layout(request):
        if entry.artifact == None:
            dirs[entry.dest] = []
            continue
        url = url_for(entry.artifact, mirror)
        dirs[entry.dest] = [url, entry.artifact.sha256, canonical_id_for(url, entry.kind), entry.artifact.directory]
    android_app_sdk_repo(
        name = APP_SDK_FILES_NAME,
        dirs = dirs,
        local_path = request.local_path,
        substitutions = app_sdk_substitutions(request),
    )
    android_app_sdk_hub_repo(
        name = APP_SDK_HUB_NAME,
        build = app_sdk_hub_build(APP_SDK_FILES_NAME, sdk_level(request)) if request.enabled else "",
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
    accepted = root_accepts(module_ctx.modules)
    for module in module_ctx.modules:
        for tag in module.tags.configure:
            requests.append(_configure_tag(tag, accepted))
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
            android_sdk_local_repo(
                name = HUB_NAME + "_local",
                api_level = request.api_level,
                extra_packages = request.extra_packages,
                local_path = request.local_path,
            )
        elif request.enabled:
            mirror = request.mirror if request.mirror != "" else DEFAULT_MIRROR
            for platform in sorted(repos["ndk"].keys()):
                _declare(android_ndk_repo, repos["ndk"][platform], NDK_ARTIFACTS[platform], "ndk", mirror)
            for platform in sorted(repos["build_tools"].keys()):
                _declare(android_build_tools_repo, repos["build_tools"][platform], BUILD_TOOLS_ARTIFACTS[platform], "build-tools", mirror)
            for level in sorted(repos["platform"].keys()):
                _declare(android_platform_repo, repos["platform"][level], PLATFORM_ARTIFACTS[level], "platform", mirror)
            for host in sorted(repos.get("platform_tools", {}).keys()):
                _declare(android_platform_tools_repo, repos["platform_tools"][host], PLATFORM_TOOLS_ARTIFACTS[host], "platform-tools", mirror)
            for host in sorted(repos.get("emulator", {}).keys()):
                _declare(android_emulator_repo, repos["emulator"][host], EMULATOR_ARTIFACTS[host], "emulator", mirror)
    _declare_app_sdk(requests[0] if len(requests) == 1 else struct(enabled = False, local_path = "", mirror = "", api_level = 0))
    platform_repos = repos.get("platform", {})
    hub_repo(
        name = HUB_NAME,
        files = {"BUILD.bazel": hub_build(
            repos.get("ndk", {}),
            repos.get("build_tools", {}),
            BUNDLETOOL_REPOSITORY,
            platform_repo = platform_repos[sorted(platform_repos.keys())[0]] if len(platform_repos) > 0 else "",
            platform_tools_repos = repos.get("platform_tools", {}),
            emulator_repos = repos.get("emulator", {}),
        )},
    )
    return module_ctx.extension_metadata(reproducible = True)

android_sdk = module_extension(
    implementation = _android_sdk_impl,
    tag_classes = {
        "license": license_tag("Accepts Android SDK licenses."),
        "configure": tag_class(attrs = {
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
