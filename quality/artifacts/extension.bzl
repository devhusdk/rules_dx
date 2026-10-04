"""Standalone quality-tool acquisition."""

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
load("//quality/artifacts:ruff.linux_arm64.bzl", _ruff_linux_arm64 = "ARTIFACT")
load("//quality/artifacts:ruff.linux_x86_64.bzl", _ruff_linux_x86_64 = "ARTIFACT")
load("//quality/artifacts:ruff.macos_arm64.bzl", _ruff_macos_arm64 = "ARTIFACT")
load("//quality/artifacts:ruff.windows_x86_64.bzl", _ruff_windows_x86_64 = "ARTIFACT")
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
    _ruff_linux_x86_64,
    _ruff_linux_arm64,
    _ruff_macos_arm64,
    _ruff_windows_x86_64,
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
    return "dx_%s_%s_%s" % (artifact["tool"], artifact["os"], artifact["cpu"])

def _sha256_of(ctx, path):
    """Returns the sha256 hex of one repo-relative file, or "" when unavailable."""
    for argv in (
        ["sha256sum", path],
        ["shasum", "-a", "256", path],
        ["python3", "-c", "import hashlib,sys; print(hashlib.sha256(open(sys.argv[1], 'rb').read()).hexdigest())", path],
    ):
        result = ctx.execute(argv)
        if result.return_code == 0:
            return result.stdout.split(" ")[0].strip()
    return ""

def _verify_executable_sha256(ctx, executable, want):
    """Fails closed unless the extracted executable matches its inner digest."""
    if want == None or want == "":
        fail("standalone tool repo: missing executable_sha256 for '" + executable +
             "' (regenerate metadata with //quality/artifacts:update)")
    got = _sha256_of(ctx, executable)
    if got == "":
        fail("standalone tool repo: cannot hash '" + executable +
             "' (need sha256sum, shasum, or python3 on PATH to verify the inner digest)")
    if got != want:
        fail("standalone tool repo: executable sha256 mismatch for '" + executable +
             "': got " + got + ", want " + want)

def _standalone_tool_repo_impl(ctx):
    kind = ctx.attr.archive_format
    if kind == "none":
        ctx.download(
            url = ctx.attr.url,
            output = ctx.attr.executable,
            sha256 = ctx.attr.sha256,
            executable = True,
            canonical_id = "dx-tool:" + ctx.attr.url,
        )
    elif kind == "gzip":
        ctx.download(
            url = ctx.attr.url,
            output = ctx.attr.asset,
            sha256 = ctx.attr.sha256,
            canonical_id = "dx-tool:" + ctx.attr.url,
        )

        ctx.extract(ctx.attr.asset)
        ctx.execute(["chmod", "755", ctx.attr.executable])
    elif kind == "tar.gz":
        ctx.download(
            url = ctx.attr.url,
            output = ctx.attr.asset,
            sha256 = ctx.attr.sha256,
            canonical_id = "dx-tool:" + ctx.attr.url,
        )

        ctx.extract(ctx.attr.asset)
    elif kind == "zip":
        ctx.download(
            url = ctx.attr.url,
            output = ctx.attr.asset,
            sha256 = ctx.attr.sha256,
            canonical_id = "dx-tool:" + ctx.attr.url,
        )

        ctx.extract(ctx.attr.asset)
    else:
        fail("unsupported archive format: " + kind)

    _verify_executable_sha256(ctx, ctx.attr.executable, ctx.attr.executable_sha256)
    ctx.file("BUILD.bazel", "\n".join([
        "filegroup(",
        '    name = "tool",',
        "    srcs = [%r]," % ctx.attr.executable,
        '    visibility = ["//visibility:public"],',
        ")",
        "",
    ]))

_standalone_tool_repo = repository_rule(
    implementation = _standalone_tool_repo_impl,
    attrs = {
        "archive_format": attr.string(mandatory = True),
        "asset": attr.string(mandatory = True),
        "executable": attr.string(mandatory = True),
        "executable_sha256": attr.string(mandatory = True),
        "sha256": attr.string(mandatory = True),
        "url": attr.string(mandatory = True),
    },
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
        platform = artifact["os"] + "_" + artifact["cpu"]
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
