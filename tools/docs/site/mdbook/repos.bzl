"""Pinned upstream mdBook acquisition for the documentation site."""

MDBOOK_VERSION = "0.4.52"

MDBOOK_RELEASE = "https://github.com/rust-lang/mdBook/releases/download/v" + MDBOOK_VERSION

_ARCHIVE_TAR_GZ = "tar.gz"
_ARCHIVE_ZIP = "zip"
_EXECUTABLE_WINDOWS = "mdbook.exe"

_HUB_NAME = "dx_mdbook"
_REPOSITORY_PREFIX = "dx_mdbook_"

ARTIFACTS = {
    "linux_x86_64": struct(
        archive_format = _ARCHIVE_TAR_GZ,
        asset = "mdbook-v" + MDBOOK_VERSION + "-x86_64-unknown-linux-gnu.tar.gz",
        executable = "mdbook",
        executable_sha256 = "29b7e5ba01321e3bbc4487012544cea516f01377ffd1e26b4adeed9b8ed875a9",
        sha256 = "c0b903f01dd8f4edc644372ad2b80b1fdddd12552d37b6a098657cbd8eddd768",
        triple = "x86_64-unknown-linux-gnu",
    ),
    "linux_arm64": struct(
        archive_format = _ARCHIVE_TAR_GZ,
        asset = "mdbook-v" + MDBOOK_VERSION + "-aarch64-unknown-linux-musl.tar.gz",
        executable = "mdbook",
        executable_sha256 = "207696773a2ce4f37c58782d0853f4179694a289cb7508eaa93e0dc595407294",
        sha256 = "7273dda980915a1e2f114d63d432aa6284551e37f0358e3ce7653d1e49e6fa3f",
        triple = "aarch64-unknown-linux-musl",
    ),
    "macos_arm64": struct(
        archive_format = _ARCHIVE_TAR_GZ,
        asset = "mdbook-v" + MDBOOK_VERSION + "-aarch64-apple-darwin.tar.gz",
        executable = "mdbook",
        executable_sha256 = "80ca3d4d6dfc1a12d231b14ece0dd6bd9cf6233f74b45f3c7ff915b6923eeac5",
        sha256 = "4aee6a8ff54dc59fbe9bcf1c38b8f58583fba63535dff7a5ec8205f0cffc3fd4",
        triple = "aarch64-apple-darwin",
    ),
    "windows_x86_64": struct(
        archive_format = _ARCHIVE_ZIP,
        asset = "mdbook-v" + MDBOOK_VERSION + "-x86_64-pc-windows-msvc.zip",
        executable = _EXECUTABLE_WINDOWS,
        executable_sha256 = "8400c017423686ad8025532baaf4b1683b8caf7147fbe6fc4cddf7a85902cbde",
        sha256 = "33510a5745c593e06f4b8e21f531d9204c6f4444846b4bd57994e82a7bdfec98",
        triple = "x86_64-pc-windows-msvc",
    ),
}

PLATFORM_CONSTRAINTS = {
    "linux_x86_64": ["@platforms//os:linux", "@platforms//cpu:x86_64"],
    "linux_arm64": ["@platforms//os:linux", "@platforms//cpu:arm64"],
    "macos_arm64": ["@platforms//os:macos", "@platforms//cpu:arm64"],
    "windows_x86_64": ["@platforms//os:windows", "@platforms//cpu:x86_64"],
}

_ARTIFACT_KEYS = [
    "archive_format",
    "asset",
    "executable",
    "executable_sha256",
    "sha256",
    "triple",
]

def repository_name(platform):
    """Returns the repository name one platform's mdBook is acquired into."""
    return _REPOSITORY_PREFIX + platform

def artifact_repositories():
    """Returns the platform-keyed mdBook repository names."""
    return {platform: repository_name(platform) for platform in sorted(ARTIFACTS.keys())}

def no_match_error():
    """Returns the diagnostic for an execution platform upstream ships no mdBook for."""
    return ("rules_dx: no pinned mdBook " + MDBOOK_VERSION + " artifact for this execution " +
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
        suffix = ".zip" if artifact.archive_format == _ARCHIVE_ZIP else ".tar.gz"
        expected = "mdbook-v" + MDBOOK_VERSION + "-" + artifact.triple + suffix
        if artifact.asset != expected:
            errors.append("artifact '" + platform + "' asset is '" + artifact.asset +
                          "', want the pinned '" + expected + "'")
    return errors

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
    lines.append('    name = "mdbook",')
    lines.append("    actual = select({")
    for platform in sorted(ARTIFACTS.keys()):
        lines.append('        ":%s": "@%s//:tool",' % (platform, repository_name(platform)))
    lines.append("    }, no_match_error = \"%s\")," % no_match_error())
    lines.append('    visibility = ["//visibility:public"],')
    lines.append(")")
    return "\n".join(lines) + "\n"

def _mdbook_repo_impl(ctx):
    ctx.download_and_extract(
        url = ctx.attr.url,
        output = "extracted",
        sha256 = ctx.attr.sha256,
        type = ctx.attr.archive_format,
        canonical_id = "dx-mdbook-archive:" + ctx.attr.url,
    )
    extracted = "extracted/" + ctx.attr.executable
    if not ctx.path(extracted).exists:
        fail("mdbook: '" + ctx.attr.asset + "' holds no '" + ctx.attr.executable + "'")
    ctx.download(
        url = "file://" + _absolute(ctx, extracted),
        output = ctx.attr.executable,
        sha256 = ctx.attr.executable_sha256,
        executable = True,
        canonical_id = "dx-mdbook-executable:" + ctx.attr.url + ":" + ctx.attr.executable_sha256,
    )
    ctx.file("BUILD.bazel", _tool_build(ctx.attr.executable))

def _tool_build(executable):
    """Returns the BUILD file exposing one acquired mdBook."""
    return "\n".join([
        "filegroup(",
        '    name = "tool",',
        "    srcs = [%r]," % executable,
        '    visibility = ["//visibility:public"],',
        ")",
        "",
    ])

def _absolute(ctx, path):
    """Returns one repository path as an absolute POSIX path."""
    text = str(ctx.path(path)).replace("\\", "/")
    return text if text.startswith("/") else "/" + text

def _hub_repo_impl(ctx):
    ctx.file("BUILD.bazel", hub_build())

_mdbook_repo = repository_rule(
    implementation = _mdbook_repo_impl,
    attrs = {
        "archive_format": attr.string(mandatory = True),
        "asset": attr.string(mandatory = True),
        "executable": attr.string(mandatory = True),
        "executable_sha256": attr.string(mandatory = True),
        "sha256": attr.string(mandatory = True),
        "url": attr.string(mandatory = True),
    },
)

_mdbook_hub_repo = repository_rule(
    implementation = _hub_repo_impl,
    attrs = {},
)

def _mdbook_impl(module_ctx):
    if len(module_ctx.modules) == 0:
        fail("mdbook: the extension is only reachable from a module graph")
    errors = artifact_errors()
    if len(errors) > 0:
        fail("mdbook: " + "; ".join(errors))
    for platform in sorted(ARTIFACTS.keys()):
        artifact = ARTIFACTS[platform]
        _mdbook_repo(
            name = repository_name(platform),
            archive_format = artifact.archive_format,
            asset = artifact.asset,
            executable = artifact.executable,
            executable_sha256 = artifact.executable_sha256,
            sha256 = artifact.sha256,
            url = MDBOOK_RELEASE + "/" + artifact.asset,
        )
    _mdbook_hub_repo(name = _HUB_NAME)

mdbook = module_extension(
    implementation = _mdbook_impl,
)
