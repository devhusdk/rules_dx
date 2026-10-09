"""Pinned upstream Wasmtime acquisition for the WASI runner."""

WASMTIME_VERSION = "49.0.2"

WASMTIME_RELEASES = "https://github.com/bytecodealliance/wasmtime/releases/download/v" + WASMTIME_VERSION

_ARCHIVE = "tar.xz"

ARTIFACTS = {
    "linux_arm64": struct(
        archive_format = _ARCHIVE,
        asset = "wasmtime-v49.0.2-aarch64-linux.tar.xz",
        directory = "wasmtime-v49.0.2-aarch64-linux",
        executable = "wasmtime",
        sha256 = "ca14988c6da3d92512bd9c3bad6cbd06b1f5743b6911a9e709f1a6fa30637d7f",
    ),
    "linux_x86_64": struct(
        archive_format = _ARCHIVE,
        asset = "wasmtime-v49.0.2-x86_64-linux.tar.xz",
        directory = "wasmtime-v49.0.2-x86_64-linux",
        executable = "wasmtime",
        sha256 = "a4d6e9e3a5a60f527cf7793d674c48930c80c2e8977995b8a275cad3254b9322",
    ),
    "macos_arm64": struct(
        archive_format = _ARCHIVE,
        asset = "wasmtime-v49.0.2-aarch64-macos.tar.xz",
        directory = "wasmtime-v49.0.2-aarch64-macos",
        executable = "wasmtime",
        sha256 = "ea5a79261caa39819b863fd850879428cf90548353e1e5cf9bcbcc4e7addf2c1",
    ),
    "macos_x86_64": struct(
        archive_format = _ARCHIVE,
        asset = "wasmtime-v49.0.2-x86_64-macos.tar.xz",
        directory = "wasmtime-v49.0.2-x86_64-macos",
        executable = "wasmtime",
        sha256 = "b24513efff6a5dbeb196e2a01097ba632a8b0ea1563ba297ada26ceb3aa067d9",
    ),
    "windows_x86_64": struct(
        archive_format = "zip",
        asset = "wasmtime-v49.0.2-x86_64-windows.zip",
        directory = "wasmtime-v49.0.2-x86_64-windows",
        executable = "wasmtime.exe",
        sha256 = "c95a3925acde6398808baabe5ad13d0b1df02b004bc256c00cf8e2a36daccea8",
    ),
}

PLATFORM_CONSTRAINTS = {
    "linux_arm64": ["@platforms//os:linux", "@platforms//cpu:arm64"],
    "linux_x86_64": ["@platforms//os:linux", "@platforms//cpu:x86_64"],
    "macos_arm64": ["@platforms//os:macos", "@platforms//cpu:arm64"],
    "macos_x86_64": ["@platforms//os:macos", "@platforms//cpu:x86_64"],
    "windows_x86_64": ["@platforms//os:windows", "@platforms//cpu:x86_64"],
}

_HUB_NAME = "dx_wasmtime"
_REPOSITORY_PREFIX = "dx_wasmtime_"

_ARTIFACT_KEYS = [
    "archive_format",
    "asset",
    "directory",
    "executable",
    "sha256",
]

def repository_name(platform):
    """Returns the repository name one platform's Wasmtime is acquired into."""
    return _REPOSITORY_PREFIX + platform

def artifact_repositories():
    """Returns the platform-keyed Wasmtime repository names."""
    return {platform: repository_name(platform) for platform in sorted(ARTIFACTS.keys())}

def no_match_error():
    """Returns the diagnostic for an execution platform upstream ships no Wasmtime for."""
    return ("rules_dx: no pinned Wasmtime " + WASMTIME_VERSION + " artifact for this execution " +
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
        expected = "wasmtime-v" + WASMTIME_VERSION + "-" + _release_triple(platform) + _archive_suffix(platform)
        if artifact.asset != expected:
            errors.append("artifact '" + platform + "' asset is '" + artifact.asset +
                          "', want the pinned '" + expected + "'")
    return errors

def _release_triple(platform):
    """Returns the upstream release triple embedded in one platform's asset name."""
    parts = platform.split("_", 1)
    arch = {"arm64": "aarch64"}.get(parts[1], parts[1])
    return arch + "-" + parts[0]

def _archive_suffix(platform):
    """Returns the upstream archive suffix for one platform."""
    if platform.startswith("windows_"):
        return ".zip"
    return ".tar.xz"

def url_for(artifact):
    """Returns the download URL one pinned Wasmtime artifact lives at."""
    return WASMTIME_RELEASES + "/" + artifact.asset

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
    lines.append('    name = "wasmtime",')
    lines.append("    actual = select({")
    for platform in sorted(ARTIFACTS.keys()):
        lines.append('        ":%s": "@%s//:wasmtime",' % (platform, repository_name(platform)))
    lines.append("    }, no_match_error = \"%s\")," % no_match_error())
    lines.append('    visibility = ["//visibility:public"],')
    lines.append(")")
    return "\n".join(lines) + "\n"

def _tool_build(executable):
    """Returns the BUILD file exposing one acquired Wasmtime binary."""
    return "\n".join([
        "filegroup(",
        '    name = "wasmtime",',
        '    srcs = ["extracted/%s"],' % executable,
        '    visibility = ["//visibility:public"],',
        ")",
        "",
    ])

def _wasmtime_repo_impl(ctx):
    ctx.download_and_extract(
        url = ctx.attr.url,
        output = "extracted",
        sha256 = ctx.attr.sha256,
        type = ctx.attr.archive_format,
        stripPrefix = ctx.attr.directory,
        canonical_id = "dx-wasmtime-archive:" + ctx.attr.url,
    )
    if not ctx.path("extracted/" + ctx.attr.executable).exists:
        fail("wasmtime: '" + ctx.attr.asset + "' holds no '" + ctx.attr.executable + "'")
    ctx.file("BUILD.bazel", _tool_build(ctx.attr.executable))

def _hub_repo_impl(ctx):
    ctx.file("BUILD.bazel", hub_build())

_wasmtime_repo = repository_rule(
    implementation = _wasmtime_repo_impl,
    attrs = {
        "archive_format": attr.string(mandatory = True),
        "asset": attr.string(mandatory = True),
        "directory": attr.string(mandatory = True),
        "executable": attr.string(mandatory = True),
        "sha256": attr.string(mandatory = True),
        "url": attr.string(mandatory = True),
    },
)

_wasmtime_hub_repo = repository_rule(
    implementation = _hub_repo_impl,
    attrs = {},
)

def _wasmtime_impl(module_ctx):
    if len(module_ctx.modules) == 0:
        fail("wasmtime: the extension is only reachable from a module graph")
    errors = artifact_errors()
    if len(errors) > 0:
        fail("wasmtime: " + "; ".join(errors))
    for platform in sorted(ARTIFACTS.keys()):
        artifact = ARTIFACTS[platform]
        _wasmtime_repo(
            name = repository_name(platform),
            archive_format = artifact.archive_format,
            asset = artifact.asset,
            directory = artifact.directory,
            executable = artifact.executable,
            sha256 = artifact.sha256,
            url = url_for(artifact),
        )
    _wasmtime_hub_repo(name = _HUB_NAME)

wasmtime = module_extension(
    implementation = _wasmtime_impl,
)
