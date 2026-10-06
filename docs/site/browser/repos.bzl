"""Pinned upstream Firefox acquisition for the documentation browser harness."""

FIREFOX_VERSION = "157.0"

FIREFOX_CDN = "https://download-installer.cdn.mozilla.net/pub/firefox/releases/" + FIREFOX_VERSION

_ARCHIVE = "tar.xz"
_EXECUTABLE = "firefox/firefox"

ARTIFACTS = {
    "linux_arm64": struct(
        archive_format = _ARCHIVE,
        asset = "firefox-" + FIREFOX_VERSION + ".tar.xz",
        directory = "linux-aarch64",
        executable = _EXECUTABLE,
        sha256 = "73fc3d6f6f4d3fcdeee59db90156568af9959405120ad686e535f572995074d0",
    ),
    "linux_x86_64": struct(
        archive_format = _ARCHIVE,
        asset = "firefox-" + FIREFOX_VERSION + ".tar.xz",
        directory = "linux-x86_64",
        executable = _EXECUTABLE,
        sha256 = "42f2c62a562316982ef5a796738c57602bf84a984f5c616e80bcff4627f78fff",
    ),
}

PLATFORM_CONSTRAINTS = {
    "linux_arm64": ["@platforms//os:linux", "@platforms//cpu:arm64"],
    "linux_x86_64": ["@platforms//os:linux", "@platforms//cpu:x86_64"],
}

_HUB_NAME = "dx_firefox"
_REPOSITORY_PREFIX = "dx_firefox_"

_ARTIFACT_KEYS = [
    "archive_format",
    "asset",
    "directory",
    "executable",
    "sha256",
]


def repository_name(platform):
    """Returns the repository name one platform's Firefox is acquired into."""
    return _REPOSITORY_PREFIX + platform


def artifact_repositories():
    """Returns the platform-keyed Firefox repository names."""
    return {platform: repository_name(platform) for platform in sorted(ARTIFACTS.keys())}


def no_match_error():
    """Returns the diagnostic for an execution platform upstream ships no Firefox for."""
    return ("rules_dx: no pinned Firefox " + FIREFOX_VERSION + " artifact for this execution " +
            "platform; want one of " + ", ".join(sorted(ARTIFACTS.keys())))


def artifact_errors():
    """Returns one error string per artifact that is not ready to download."""
    errors = []
    for platform in sorted(ARTIFACTS.keys()):
        if platform not in PLATFORM_CONSTRAINTS:
            errors.append("artifact '" + platform + "' names no platform constraints")
        artifact = ARTIFACTS[platform]
        missing = [key for key in _ARTIFACT_KEYS if getattr(artifact, key, "") == ""]
        if len(missing) > 0:
            errors.append("artifact '" + platform + "' is missing " + ", ".join(missing))
            continue
        expected = "firefox-" + FIREFOX_VERSION + ".tar.xz"
        if artifact.asset != expected:
            errors.append("artifact '" + platform + "' asset is '" + artifact.asset +
                          "', want the pinned '" + expected + "'")
    return errors


def url_for(artifact):
    """Returns the download URL one pinned Firefox artifact lives at."""
    return FIREFOX_CDN + "/" + artifact.directory + "/en-US/" + artifact.asset


def hub_build():
    """Returns the hub BUILD file selecting one pinned artifact per platform."""
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
    lines.append("alias(")
    lines.append('    name = "firefox",')
    lines.append("    actual = select({")
    for platform in sorted(ARTIFACTS.keys()):
        lines.append('        ":%s": "@%s//:binary",' % (platform, repository_name(platform)))
    lines.append("    }, no_match_error = \"%s\")," % no_match_error())
    lines.append('    visibility = ["//visibility:public"],')
    lines.append(")")
    lines.append("")
    lines.append("alias(")
    lines.append('    name = "tree",')
    lines.append("    actual = select({")
    for platform in sorted(ARTIFACTS.keys()):
        lines.append('        ":%s": "@%s//:tool",' % (platform, repository_name(platform)))
    lines.append("    }, no_match_error = \"%s\")," % no_match_error())
    lines.append('    visibility = ["//visibility:public"],')
    lines.append(")")
    return "\n".join(lines) + "\n"


def _firefox_repo_impl(ctx):
    ctx.download_and_extract(
        url = ctx.attr.url,
        output = "extracted",
        sha256 = ctx.attr.sha256,
        type = ctx.attr.archive_format,
        canonical_id = "dx-firefox-archive:" + ctx.attr.url,
    )
    if not ctx.path("extracted/" + ctx.attr.executable).exists:
        fail("firefox: '" + ctx.attr.asset + "' holds no '" + ctx.attr.executable + "'")
    ctx.file("BUILD.bazel", _tool_build())

def _tool_build():
    """Returns the BUILD file exposing one acquired Firefox tree."""
    return "\n".join([
        "filegroup(",
        '    name = "binary",',
        '    srcs = ["extracted/%s"],' % _EXECUTABLE,
        '    visibility = ["//visibility:public"],',
        ")",
        "",
        "filegroup(",
        '    name = "tool",',
        "    srcs = glob([",
        '        "extracted/**",',
        "    ]),",
        '    visibility = ["//visibility:public"],',
        ")",
        "",
    ])


def _hub_repo_impl(ctx):
    ctx.file("BUILD.bazel", hub_build())

_firefox_repo = repository_rule(
    implementation = _firefox_repo_impl,
    attrs = {
        "archive_format": attr.string(mandatory = True),
        "asset": attr.string(mandatory = True),
        "executable": attr.string(mandatory = True),
        "sha256": attr.string(mandatory = True),
        "url": attr.string(mandatory = True),
    },
)

_firefox_hub_repo = repository_rule(
    implementation = _hub_repo_impl,
    attrs = {},
)

def _firefox_impl(module_ctx):
    if len(module_ctx.modules) == 0:
        fail("firefox: the extension is only reachable from a module graph")
    errors = artifact_errors()
    if len(errors) > 0:
        fail("firefox: " + "; ".join(errors))
    for platform in sorted(ARTIFACTS.keys()):
        artifact = ARTIFACTS[platform]
        _firefox_repo(
            name = repository_name(platform),
            archive_format = artifact.archive_format,
            asset = artifact.asset,
            executable = artifact.executable,
            sha256 = artifact.sha256,
            url = url_for(artifact),
        )
    _firefox_hub_repo(name = _HUB_NAME)

firefox = module_extension(
    implementation = _firefox_impl,
)