"""Standalone quality-tool acquisition."""

load("//quality/artifacts:acquire.bzl", "ACQUIRE_ATTRS", "acquire_tool")
load("//quality/artifacts:actionlint.linux_arm64.bzl", _actionlint_linux_arm64 = "ARTIFACT")
load("//quality/artifacts:actionlint.linux_x86_64.bzl", _actionlint_linux_x86_64 = "ARTIFACT")
load("//quality/artifacts:actionlint.macos_arm64.bzl", _actionlint_macos_arm64 = "ARTIFACT")
load("//quality/artifacts:actionlint.windows_x86_64.bzl", _actionlint_windows_x86_64 = "ARTIFACT")
load("//quality/artifacts:biome.linux_arm64.bzl", _biome_linux_arm64 = "ARTIFACT")
load("//quality/artifacts:biome.linux_x86_64.bzl", _biome_linux_x86_64 = "ARTIFACT")
load("//quality/artifacts:biome.macos_arm64.bzl", _biome_macos_arm64 = "ARTIFACT")
load("//quality/artifacts:biome.windows_x86_64.bzl", _biome_windows_x86_64 = "ARTIFACT")
load("//quality/artifacts:buildifier.linux_arm64.bzl", _buildifier_linux_arm64 = "ARTIFACT")
load("//quality/artifacts:buildifier.linux_x86_64.bzl", _buildifier_linux_x86_64 = "ARTIFACT")
load("//quality/artifacts:buildifier.macos_arm64.bzl", _buildifier_macos_arm64 = "ARTIFACT")
load("//quality/artifacts:buildifier.windows_x86_64.bzl", _buildifier_windows_x86_64 = "ARTIFACT")
load("//quality/artifacts:gitleaks.linux_arm64.bzl", _gitleaks_linux_arm64 = "ARTIFACT")
load("//quality/artifacts:gitleaks.linux_x86_64.bzl", _gitleaks_linux_x86_64 = "ARTIFACT")
load("//quality/artifacts:gitleaks.macos_arm64.bzl", _gitleaks_macos_arm64 = "ARTIFACT")
load("//quality/artifacts:gitleaks.windows_x86_64.bzl", _gitleaks_windows_x86_64 = "ARTIFACT")
load("//quality/artifacts:hub.bzl", "artifact_map_errors", "artifact_metadata_errors", "decode_artifacts", "encode_artifacts", "hub_build")
load("//quality/artifacts:keep_sorted.linux_arm64.bzl", _keep_sorted_linux_arm64 = "ARTIFACT")
load("//quality/artifacts:keep_sorted.linux_x86_64.bzl", _keep_sorted_linux_x86_64 = "ARTIFACT")
load("//quality/artifacts:keep_sorted.macos_arm64.bzl", _keep_sorted_macos_arm64 = "ARTIFACT")
load("//quality/artifacts:keep_sorted.windows_x86_64.bzl", _keep_sorted_windows_x86_64 = "ARTIFACT")
load("//quality/artifacts:platforms.bzl", "artifact_platform_key")
load("//quality/artifacts:ruff.linux_arm64.bzl", _ruff_linux_arm64 = "ARTIFACT")
load("//quality/artifacts:ruff.linux_x86_64.bzl", _ruff_linux_x86_64 = "ARTIFACT")
load("//quality/artifacts:ruff.macos_arm64.bzl", _ruff_macos_arm64 = "ARTIFACT")
load("//quality/artifacts:ruff.windows_x86_64.bzl", _ruff_windows_x86_64 = "ARTIFACT")
load("//quality/artifacts:shellcheck.linux_arm64.bzl", _shellcheck_linux_arm64 = "ARTIFACT")
load("//quality/artifacts:shellcheck.linux_x86_64.bzl", _shellcheck_linux_x86_64 = "ARTIFACT")
load("//quality/artifacts:shellcheck.macos_arm64.bzl", _shellcheck_macos_arm64 = "ARTIFACT")
load("//quality/artifacts:shellcheck.windows_x86_64.bzl", _shellcheck_windows_x86_64 = "ARTIFACT")
load("//quality/artifacts:taplo.linux_arm64.bzl", _taplo_linux_arm64 = "ARTIFACT")
load("//quality/artifacts:taplo.linux_x86_64.bzl", _taplo_linux_x86_64 = "ARTIFACT")
load("//quality/artifacts:taplo.macos_arm64.bzl", _taplo_macos_arm64 = "ARTIFACT")
load("//quality/artifacts:taplo.windows_x86_64.bzl", _taplo_windows_x86_64 = "ARTIFACT")
load("//quality/artifacts:ty.linux_arm64.bzl", _ty_linux_arm64 = "ARTIFACT")
load("//quality/artifacts:ty.linux_x86_64.bzl", _ty_linux_x86_64 = "ARTIFACT")
load("//quality/artifacts:ty.macos_arm64.bzl", _ty_macos_arm64 = "ARTIFACT")
load("//quality/artifacts:ty.windows_x86_64.bzl", _ty_windows_x86_64 = "ARTIFACT")
load("//quality/artifacts:vale.linux_arm64.bzl", _vale_linux_arm64 = "ARTIFACT")
load("//quality/artifacts:vale.linux_x86_64.bzl", _vale_linux_x86_64 = "ARTIFACT")
load("//quality/artifacts:vale.macos_arm64.bzl", _vale_macos_arm64 = "ARTIFACT")
load("//quality/artifacts:vale.windows_x86_64.bzl", _vale_windows_x86_64 = "ARTIFACT")

_ARTIFACTS = [
    _actionlint_linux_x86_64,
    _actionlint_linux_arm64,
    _actionlint_macos_arm64,
    _actionlint_windows_x86_64,
    _biome_linux_x86_64,
    _biome_linux_arm64,
    _biome_macos_arm64,
    _biome_windows_x86_64,
    _buildifier_linux_x86_64,
    _buildifier_linux_arm64,
    _buildifier_macos_arm64,
    _buildifier_windows_x86_64,
    _gitleaks_linux_x86_64,
    _gitleaks_linux_arm64,
    _gitleaks_macos_arm64,
    _gitleaks_windows_x86_64,
    _keep_sorted_linux_x86_64,
    _keep_sorted_linux_arm64,
    _keep_sorted_macos_arm64,
    _keep_sorted_windows_x86_64,
    _ruff_linux_x86_64,
    _ruff_linux_arm64,
    _ruff_macos_arm64,
    _ruff_windows_x86_64,
    _shellcheck_linux_x86_64,
    _shellcheck_linux_arm64,
    _shellcheck_macos_arm64,
    _shellcheck_windows_x86_64,
    _taplo_linux_x86_64,
    _taplo_linux_arm64,
    _taplo_macos_arm64,
    _taplo_windows_x86_64,
    _ty_linux_x86_64,
    _ty_linux_arm64,
    _ty_macos_arm64,
    _ty_windows_x86_64,
    _vale_linux_x86_64,
    _vale_linux_arm64,
    _vale_macos_arm64,
    _vale_windows_x86_64,
]

TOOL_ARTIFACTS = _ARTIFACTS

def _repo_name(artifact):
    return "dx_%s_%s" % (artifact["tool"], artifact_platform_key(artifact))

def _standalone_tool_repo_impl(ctx):
    acquire_tool(ctx)

_standalone_tool_repo = repository_rule(
    implementation = _standalone_tool_repo_impl,
    attrs = ACQUIRE_ATTRS,
)

def _hub_repo_impl(ctx):
    decoded = decode_artifacts(ctx.attr.artifacts)
    if decoded.error != "":
        fail("dx_tools hub: " + decoded.error)
    errors = artifact_map_errors(decoded.artifacts)
    if len(errors) > 0:
        fail("dx_tools hub: " + "; ".join(errors))
    ctx.file("BUILD.bazel", hub_build(decoded.artifacts, {
        "cpu_arm64": ctx.attr.cpu_arm64,
        "cpu_x86_64": ctx.attr.cpu_x86_64,
        "os_linux": ctx.attr.os_linux,
        "os_macos": ctx.attr.os_macos,
        "os_windows": ctx.attr.os_windows,
    }))

_hub_repo = repository_rule(
    implementation = _hub_repo_impl,
    attrs = {
        "artifacts": attr.string(mandatory = True),
        "cpu_arm64": attr.string(mandatory = True),
        "cpu_x86_64": attr.string(mandatory = True),
        "os_linux": attr.string(mandatory = True),
        "os_macos": attr.string(mandatory = True),
        "os_windows": attr.string(mandatory = True),
    },
)

def _dx_tools_impl(ctx):
    os_linux, os_macos, os_windows, cpu_x86_64, cpu_arm64 = None, None, None, None, None
    for module in ctx.modules:
        for tag in module.tags.platform:
            if os_linux != None:
                fail("dx_tools.platform may be declared at most once")
            os_linux = str(tag.os_linux)
            os_macos = str(tag.os_macos)
            os_windows = str(tag.os_windows)
            cpu_x86_64 = str(tag.cpu_x86_64)
            cpu_arm64 = str(tag.cpu_arm64)
    if os_linux == None:
        fail("dx_tools.platform(os_linux, os_macos, os_windows, cpu_x86_64, cpu_arm64) is required in MODULE.bazel")
    errors = artifact_metadata_errors(_ARTIFACTS)
    if len(errors) > 0:
        fail("dx_tools: " + "; ".join(errors))
    by_tool = {}
    for artifact in _ARTIFACTS:
        name = _repo_name(artifact)
        _standalone_tool_repo(
            name = name,
            url = artifact["url"],
            sha256 = artifact["sha256"],
            asset = artifact["url"].split("/")[-1],
            archive_format = artifact["archive"]["format"],
            executable = artifact["executable"],
            executable_sha256 = artifact["executable_sha256"],
        )
        platform = artifact_platform_key(artifact)
        by_tool.setdefault(artifact["tool"], {})[platform] = name
    _hub_repo(
        name = "dx_tools",
        artifacts = encode_artifacts(by_tool),
        os_linux = os_linux,
        os_macos = os_macos,
        os_windows = os_windows,
        cpu_x86_64 = cpu_x86_64,
        cpu_arm64 = cpu_arm64,
    )

_platform = tag_class(attrs = {
    "cpu_arm64": attr.label(mandatory = True),
    "cpu_x86_64": attr.label(mandatory = True),
    "os_linux": attr.label(mandatory = True),
    "os_macos": attr.label(mandatory = True),
    "os_windows": attr.label(mandatory = True),
})

dx_tools = module_extension(
    implementation = _dx_tools_impl,
    tag_classes = {"platform": _platform},
)
