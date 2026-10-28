"""Standalone tool acquisition."""

load("//quality/artifacts:acquire.bzl", "ACQUIRE_ATTRS", "acquire_tool")
load("//quality/artifacts:actionlint.linux_arm64.bzl", _actionlint_linux_arm64 = "ARTIFACT")
load("//quality/artifacts:actionlint.linux_x86_64.bzl", _actionlint_linux_x86_64 = "ARTIFACT")
load("//quality/artifacts:actionlint.macos_arm64.bzl", _actionlint_macos_arm64 = "ARTIFACT")
load("//quality/artifacts:actionlint.windows_arm64.bzl", _actionlint_windows_arm64 = "ARTIFACT")
load("//quality/artifacts:actionlint.windows_x86_64.bzl", _actionlint_windows_x86_64 = "ARTIFACT")
load("//quality/artifacts:biome.linux_arm64.bzl", _biome_linux_arm64 = "ARTIFACT")
load("//quality/artifacts:biome.linux_x86_64.bzl", _biome_linux_x86_64 = "ARTIFACT")
load("//quality/artifacts:biome.macos_arm64.bzl", _biome_macos_arm64 = "ARTIFACT")
load("//quality/artifacts:biome.windows_arm64.bzl", _biome_windows_arm64 = "ARTIFACT")
load("//quality/artifacts:biome.windows_x86_64.bzl", _biome_windows_x86_64 = "ARTIFACT")
load("//quality/artifacts:buf.linux_arm64.bzl", _buf_linux_arm64 = "ARTIFACT")
load("//quality/artifacts:buf.linux_x86_64.bzl", _buf_linux_x86_64 = "ARTIFACT")
load("//quality/artifacts:buf.macos_arm64.bzl", _buf_macos_arm64 = "ARTIFACT")
load("//quality/artifacts:buf.windows_arm64.bzl", _buf_windows_arm64 = "ARTIFACT")
load("//quality/artifacts:buf.windows_x86_64.bzl", _buf_windows_x86_64 = "ARTIFACT")
load("//quality/artifacts:buildifier.linux_arm64.bzl", _buildifier_linux_arm64 = "ARTIFACT")
load("//quality/artifacts:buildifier.linux_x86_64.bzl", _buildifier_linux_x86_64 = "ARTIFACT")
load("//quality/artifacts:buildifier.macos_arm64.bzl", _buildifier_macos_arm64 = "ARTIFACT")
load("//quality/artifacts:buildifier.windows_arm64.bzl", _buildifier_windows_arm64 = "ARTIFACT")
load("//quality/artifacts:buildifier.windows_x86_64.bzl", _buildifier_windows_x86_64 = "ARTIFACT")
load("//quality/artifacts:cue.linux_arm64.bzl", _cue_linux_arm64 = "ARTIFACT")
load("//quality/artifacts:cue.linux_x86_64.bzl", _cue_linux_x86_64 = "ARTIFACT")
load("//quality/artifacts:cue.macos_arm64.bzl", _cue_macos_arm64 = "ARTIFACT")
load("//quality/artifacts:cue.windows_arm64.bzl", _cue_windows_arm64 = "ARTIFACT")
load("//quality/artifacts:cue.windows_x86_64.bzl", _cue_windows_x86_64 = "ARTIFACT")
load("//quality/artifacts:gitleaks.linux_arm64.bzl", _gitleaks_linux_arm64 = "ARTIFACT")
load("//quality/artifacts:gitleaks.linux_x86_64.bzl", _gitleaks_linux_x86_64 = "ARTIFACT")
load("//quality/artifacts:gitleaks.macos_arm64.bzl", _gitleaks_macos_arm64 = "ARTIFACT")
load("//quality/artifacts:gitleaks.windows_arm64.bzl", _gitleaks_windows_arm64 = "ARTIFACT")
load("//quality/artifacts:gitleaks.windows_x86_64.bzl", _gitleaks_windows_x86_64 = "ARTIFACT")
load("//quality/artifacts:checkmake.linux_arm64.bzl", _checkmake_linux_arm64 = "ARTIFACT")
load("//quality/artifacts:checkmake.linux_x86_64.bzl", _checkmake_linux_x86_64 = "ARTIFACT")
load("//quality/artifacts:checkmake.macos_arm64.bzl", _checkmake_macos_arm64 = "ARTIFACT")
load("//quality/artifacts:checkmake.windows_arm64.bzl", _checkmake_windows_arm64 = "ARTIFACT")
load("//quality/artifacts:checkmake.windows_x86_64.bzl", _checkmake_windows_x86_64 = "ARTIFACT")
load("//quality/artifacts:tflint.linux_arm64.bzl", _tflint_linux_arm64 = "ARTIFACT")
load("//quality/artifacts:tflint.linux_x86_64.bzl", _tflint_linux_x86_64 = "ARTIFACT")
load("//quality/artifacts:tflint.macos_arm64.bzl", _tflint_macos_arm64 = "ARTIFACT")
load("//quality/artifacts:tflint.windows_x86_64.bzl", _tflint_windows_x86_64 = "ARTIFACT")
load("//quality/artifacts:hadolint.linux_arm64.bzl", _hadolint_linux_arm64 = "ARTIFACT")
load("//quality/artifacts:hadolint.linux_x86_64.bzl", _hadolint_linux_x86_64 = "ARTIFACT")
load("//quality/artifacts:hadolint.macos_arm64.bzl", _hadolint_macos_arm64 = "ARTIFACT")
load("//quality/artifacts:hadolint.windows_x86_64.bzl", _hadolint_windows_x86_64 = "ARTIFACT")
load("//quality/artifacts:hub.bzl", "artifact_map_errors", "artifact_metadata_errors", "decode_artifacts", "encode_artifacts", "hub_build")
load("//quality/artifacts:jsonnetfmt.linux_arm64.bzl", _jsonnetfmt_linux_arm64 = "ARTIFACT")
load("//quality/artifacts:jsonnetfmt.linux_x86_64.bzl", _jsonnetfmt_linux_x86_64 = "ARTIFACT")
load("//quality/artifacts:jsonnetfmt.macos_arm64.bzl", _jsonnetfmt_macos_arm64 = "ARTIFACT")
load("//quality/artifacts:jsonnetfmt.windows_arm64.bzl", _jsonnetfmt_windows_arm64 = "ARTIFACT")
load("//quality/artifacts:jsonnetfmt.windows_x86_64.bzl", _jsonnetfmt_windows_x86_64 = "ARTIFACT")
load("//quality/artifacts:helm.linux_arm64.bzl", _helm_linux_arm64 = "ARTIFACT")
load("//quality/artifacts:helm.linux_x86_64.bzl", _helm_linux_x86_64 = "ARTIFACT")
load("//quality/artifacts:helm.macos_arm64.bzl", _helm_macos_arm64 = "ARTIFACT")
load("//quality/artifacts:helm.windows_arm64.bzl", _helm_windows_arm64 = "ARTIFACT")
load("//quality/artifacts:helm.windows_x86_64.bzl", _helm_windows_x86_64 = "ARTIFACT")
load("//quality/artifacts:keep_sorted.linux_arm64.bzl", _keep_sorted_linux_arm64 = "ARTIFACT")
load("//quality/artifacts:keep_sorted.linux_x86_64.bzl", _keep_sorted_linux_x86_64 = "ARTIFACT")
load("//quality/artifacts:keep_sorted.macos_arm64.bzl", _keep_sorted_macos_arm64 = "ARTIFACT")
load("//quality/artifacts:keep_sorted.windows_x86_64.bzl", _keep_sorted_windows_x86_64 = "ARTIFACT")
load("//quality/artifacts:kubeconform.linux_arm64.bzl", _kubeconform_linux_arm64 = "ARTIFACT")
load("//quality/artifacts:kubeconform.linux_x86_64.bzl", _kubeconform_linux_x86_64 = "ARTIFACT")
load("//quality/artifacts:kubeconform.macos_arm64.bzl", _kubeconform_macos_arm64 = "ARTIFACT")
load("//quality/artifacts:kubeconform.windows_arm64.bzl", _kubeconform_windows_arm64 = "ARTIFACT")
load("//quality/artifacts:kubeconform.windows_x86_64.bzl", _kubeconform_windows_x86_64 = "ARTIFACT")
load("//quality/artifacts:mdbook.linux_arm64.bzl", _mdbook_linux_arm64 = "ARTIFACT")
load("//quality/artifacts:mdbook.linux_x86_64.bzl", _mdbook_linux_x86_64 = "ARTIFACT")
load("//quality/artifacts:mdbook.macos_arm64.bzl", _mdbook_macos_arm64 = "ARTIFACT")
load("//quality/artifacts:mdbook.windows_x86_64.bzl", _mdbook_windows_x86_64 = "ARTIFACT")
load("//quality/artifacts:nixfmt.linux_x86_64.bzl", _nixfmt_linux_x86_64 = "ARTIFACT")
load("//quality/artifacts:ormolu.linux_x86_64.bzl", _ormolu_linux_x86_64 = "ARTIFACT")
load("//quality/artifacts:ormolu.windows_x86_64.bzl", _ormolu_windows_x86_64 = "ARTIFACT")
load("//quality/artifacts:opa.linux_arm64.bzl", _opa_linux_arm64 = "ARTIFACT")
load("//quality/artifacts:opa.linux_x86_64.bzl", _opa_linux_x86_64 = "ARTIFACT")
load("//quality/artifacts:opa.macos_arm64.bzl", _opa_macos_arm64 = "ARTIFACT")
load("//quality/artifacts:opa.windows_x86_64.bzl", _opa_windows_x86_64 = "ARTIFACT")
load("//quality/artifacts:regal.linux_arm64.bzl", _regal_linux_arm64 = "ARTIFACT")
load("//quality/artifacts:regal.linux_x86_64.bzl", _regal_linux_x86_64 = "ARTIFACT")
load("//quality/artifacts:regal.macos_arm64.bzl", _regal_macos_arm64 = "ARTIFACT")
load("//quality/artifacts:regal.windows_x86_64.bzl", _regal_windows_x86_64 = "ARTIFACT")
load("//quality/artifacts:pkl.linux_arm64.bzl", _pkl_linux_arm64 = "ARTIFACT")
load("//quality/artifacts:pkl.linux_x86_64.bzl", _pkl_linux_x86_64 = "ARTIFACT")
load("//quality/artifacts:pkl.macos_arm64.bzl", _pkl_macos_arm64 = "ARTIFACT")
load("//quality/artifacts:pkl.windows_x86_64.bzl", _pkl_windows_x86_64 = "ARTIFACT")
load("//quality/artifacts:platforms.bzl", "artifact_platform_key")
load("//quality/artifacts:ruff.linux_arm64.bzl", _ruff_linux_arm64 = "ARTIFACT")
load("//quality/artifacts:ruff.linux_x86_64.bzl", _ruff_linux_x86_64 = "ARTIFACT")
load("//quality/artifacts:ruff.macos_arm64.bzl", _ruff_macos_arm64 = "ARTIFACT")
load("//quality/artifacts:ruff.windows_arm64.bzl", _ruff_windows_arm64 = "ARTIFACT")
load("//quality/artifacts:ruff.windows_x86_64.bzl", _ruff_windows_x86_64 = "ARTIFACT")
load("//quality/artifacts:scalafmt.linux_arm64.bzl", _scalafmt_linux_arm64 = "ARTIFACT")
load("//quality/artifacts:scalafmt.linux_x86_64.bzl", _scalafmt_linux_x86_64 = "ARTIFACT")
load("//quality/artifacts:scalafmt.macos_arm64.bzl", _scalafmt_macos_arm64 = "ARTIFACT")
load("//quality/artifacts:scalafmt.windows_x86_64.bzl", _scalafmt_windows_x86_64 = "ARTIFACT")
load("//quality/artifacts:shellcheck.linux_arm64.bzl", _shellcheck_linux_arm64 = "ARTIFACT")
load("//quality/artifacts:shellcheck.linux_x86_64.bzl", _shellcheck_linux_x86_64 = "ARTIFACT")
load("//quality/artifacts:shellcheck.macos_arm64.bzl", _shellcheck_macos_arm64 = "ARTIFACT")
load("//quality/artifacts:shellcheck.windows_x86_64.bzl", _shellcheck_windows_x86_64 = "ARTIFACT")
load("//quality/artifacts:shfmt.linux_arm64.bzl", _shfmt_linux_arm64 = "ARTIFACT")
load("//quality/artifacts:shfmt.linux_x86_64.bzl", _shfmt_linux_x86_64 = "ARTIFACT")
load("//quality/artifacts:shfmt.macos_arm64.bzl", _shfmt_macos_arm64 = "ARTIFACT")
load("//quality/artifacts:shfmt.windows_x86_64.bzl", _shfmt_windows_x86_64 = "ARTIFACT")
load("//quality/artifacts:stylua.linux_arm64.bzl", _stylua_linux_arm64 = "ARTIFACT")
load("//quality/artifacts:stylua.linux_x86_64.bzl", _stylua_linux_x86_64 = "ARTIFACT")
load("//quality/artifacts:stylua.macos_arm64.bzl", _stylua_macos_arm64 = "ARTIFACT")
load("//quality/artifacts:stylua.windows_x86_64.bzl", _stylua_windows_x86_64 = "ARTIFACT")
load("//quality/artifacts:taplo.linux_arm64.bzl", _taplo_linux_arm64 = "ARTIFACT")
load("//quality/artifacts:taplo.linux_x86_64.bzl", _taplo_linux_x86_64 = "ARTIFACT")
load("//quality/artifacts:taplo.macos_arm64.bzl", _taplo_macos_arm64 = "ARTIFACT")
load("//quality/artifacts:taplo.windows_arm64.bzl", _taplo_windows_arm64 = "ARTIFACT")
load("//quality/artifacts:taplo.windows_x86_64.bzl", _taplo_windows_x86_64 = "ARTIFACT")
load("//quality/artifacts:terraform.linux_arm64.bzl", _terraform_linux_arm64 = "ARTIFACT")
load("//quality/artifacts:terraform.linux_x86_64.bzl", _terraform_linux_x86_64 = "ARTIFACT")
load("//quality/artifacts:terraform.macos_arm64.bzl", _terraform_macos_arm64 = "ARTIFACT")
load("//quality/artifacts:terraform.windows_arm64.bzl", _terraform_windows_arm64 = "ARTIFACT")
load("//quality/artifacts:terraform.windows_x86_64.bzl", _terraform_windows_x86_64 = "ARTIFACT")
load("//quality/artifacts:latexindent.linux_arm64.bzl", _latexindent_linux_arm64 = "ARTIFACT")
load("//quality/artifacts:latexindent.linux_x86_64.bzl", _latexindent_linux_x86_64 = "ARTIFACT")
load("//quality/artifacts:latexindent.macos_arm64.bzl", _latexindent_macos_arm64 = "ARTIFACT")
load("//quality/artifacts:latexindent.windows_x86_64.bzl", _latexindent_windows_x86_64 = "ARTIFACT")
load("//quality/artifacts:ty.linux_arm64.bzl", _ty_linux_arm64 = "ARTIFACT")
load("//quality/artifacts:ty.linux_x86_64.bzl", _ty_linux_x86_64 = "ARTIFACT")
load("//quality/artifacts:ty.macos_arm64.bzl", _ty_macos_arm64 = "ARTIFACT")
load("//quality/artifacts:ty.windows_arm64.bzl", _ty_windows_arm64 = "ARTIFACT")
load("//quality/artifacts:ty.windows_x86_64.bzl", _ty_windows_x86_64 = "ARTIFACT")
load("//quality/artifacts:typstyle.linux_arm64.bzl", _typstyle_linux_arm64 = "ARTIFACT")
load("//quality/artifacts:typstyle.linux_x86_64.bzl", _typstyle_linux_x86_64 = "ARTIFACT")
load("//quality/artifacts:typstyle.macos_arm64.bzl", _typstyle_macos_arm64 = "ARTIFACT")
load("//quality/artifacts:typstyle.windows_arm64.bzl", _typstyle_windows_arm64 = "ARTIFACT")
load("//quality/artifacts:typstyle.windows_x86_64.bzl", _typstyle_windows_x86_64 = "ARTIFACT")
load("//quality/artifacts:uv.linux_arm64.bzl", _uv_linux_arm64 = "ARTIFACT")
load("//quality/artifacts:uv.linux_x86_64.bzl", _uv_linux_x86_64 = "ARTIFACT")
load("//quality/artifacts:uv.macos_arm64.bzl", _uv_macos_arm64 = "ARTIFACT")
load("//quality/artifacts:uv.windows_arm64.bzl", _uv_windows_arm64 = "ARTIFACT")
load("//quality/artifacts:uv.windows_x86_64.bzl", _uv_windows_x86_64 = "ARTIFACT")
load("//quality/artifacts:vale.linux_arm64.bzl", _vale_linux_arm64 = "ARTIFACT")
load("//quality/artifacts:vale.linux_x86_64.bzl", _vale_linux_x86_64 = "ARTIFACT")
load("//quality/artifacts:vale.macos_arm64.bzl", _vale_macos_arm64 = "ARTIFACT")
load("//quality/artifacts:vale.windows_arm64.bzl", _vale_windows_arm64 = "ARTIFACT")
load("//quality/artifacts:vale.windows_x86_64.bzl", _vale_windows_x86_64 = "ARTIFACT")
load("//quality/artifacts:wasmtime.linux_arm64.bzl", _wasmtime_linux_arm64 = "ARTIFACT")
load("//quality/artifacts:wasmtime.linux_x86_64.bzl", _wasmtime_linux_x86_64 = "ARTIFACT")
load("//quality/artifacts:wasmtime.macos_arm64.bzl", _wasmtime_macos_arm64 = "ARTIFACT")
load("//quality/artifacts:wasmtime.macos_x86_64.bzl", _wasmtime_macos_x86_64 = "ARTIFACT")
load("//quality/artifacts:wasmtime.windows_arm64.bzl", _wasmtime_windows_arm64 = "ARTIFACT")
load("//quality/artifacts:wasmtime.windows_x86_64.bzl", _wasmtime_windows_x86_64 = "ARTIFACT")
load("//quality/artifacts:yamlfmt.linux_arm64.bzl", _yamlfmt_linux_arm64 = "ARTIFACT")
load("//quality/artifacts:yamlfmt.linux_x86_64.bzl", _yamlfmt_linux_x86_64 = "ARTIFACT")
load("//quality/artifacts:yamlfmt.macos_arm64.bzl", _yamlfmt_macos_arm64 = "ARTIFACT")
load("//quality/artifacts:yamlfmt.windows_arm64.bzl", _yamlfmt_windows_arm64 = "ARTIFACT")
load("//quality/artifacts:yamlfmt.windows_x86_64.bzl", _yamlfmt_windows_x86_64 = "ARTIFACT")

_ARTIFACTS = [
    _actionlint_linux_x86_64,
    _actionlint_linux_arm64,
    _actionlint_macos_arm64,
    _actionlint_windows_x86_64,
    _actionlint_windows_arm64,
    _biome_linux_x86_64,
    _biome_linux_arm64,
    _biome_macos_arm64,
    _biome_windows_x86_64,
    _biome_windows_arm64,
    _buf_linux_x86_64,
    _buf_linux_arm64,
    _buf_macos_arm64,
    _buf_windows_x86_64,
    _buf_windows_arm64,
    _buildifier_linux_x86_64,
    _buildifier_linux_arm64,
    _buildifier_macos_arm64,
    _buildifier_windows_x86_64,
    _buildifier_windows_arm64,
    _cue_linux_x86_64,
    _cue_linux_arm64,
    _cue_macos_arm64,
    _cue_windows_x86_64,
    _cue_windows_arm64,
    _gitleaks_linux_x86_64,
    _gitleaks_linux_arm64,
    _gitleaks_macos_arm64,
    _gitleaks_windows_x86_64,
    _gitleaks_windows_arm64,
    _checkmake_linux_x86_64,
    _checkmake_linux_arm64,
    _checkmake_macos_arm64,
    _checkmake_windows_x86_64,
    _checkmake_windows_arm64,
    _tflint_linux_x86_64,
    _tflint_linux_arm64,
    _tflint_macos_arm64,
    _tflint_windows_x86_64,
    _hadolint_linux_x86_64,
    _hadolint_linux_arm64,
    _hadolint_macos_arm64,
    _hadolint_windows_x86_64,
    _jsonnetfmt_linux_x86_64,
    _jsonnetfmt_linux_arm64,
    _jsonnetfmt_macos_arm64,
    _jsonnetfmt_windows_x86_64,
    _jsonnetfmt_windows_arm64,
    _helm_linux_x86_64,
    _helm_linux_arm64,
    _helm_macos_arm64,
    _helm_windows_x86_64,
    _helm_windows_arm64,
    _keep_sorted_linux_x86_64,
    _keep_sorted_linux_arm64,
    _keep_sorted_macos_arm64,
    _keep_sorted_windows_x86_64,
    _kubeconform_linux_x86_64,
    _kubeconform_linux_arm64,
    _kubeconform_macos_arm64,
    _kubeconform_windows_x86_64,
    _kubeconform_windows_arm64,
    _mdbook_linux_x86_64,
    _mdbook_linux_arm64,
    _mdbook_macos_arm64,
    _mdbook_windows_x86_64,
    _nixfmt_linux_x86_64,
    _ormolu_linux_x86_64,
    _ormolu_windows_x86_64,
    _opa_linux_x86_64,
    _opa_linux_arm64,
    _opa_macos_arm64,
    _opa_windows_x86_64,
    _regal_linux_x86_64,
    _regal_linux_arm64,
    _regal_macos_arm64,
    _regal_windows_x86_64,
    _pkl_linux_x86_64,
    _pkl_linux_arm64,
    _pkl_macos_arm64,
    _pkl_windows_x86_64,
    _ruff_linux_x86_64,
    _ruff_linux_arm64,
    _ruff_macos_arm64,
    _ruff_windows_x86_64,
    _ruff_windows_arm64,
    _scalafmt_linux_x86_64,
    _scalafmt_linux_arm64,
    _scalafmt_macos_arm64,
    _scalafmt_windows_x86_64,
    _shellcheck_linux_x86_64,
    _shellcheck_linux_arm64,
    _shellcheck_macos_arm64,
    _shellcheck_windows_x86_64,
    _shfmt_linux_x86_64,
    _shfmt_linux_arm64,
    _shfmt_macos_arm64,
    _shfmt_windows_x86_64,
    _stylua_linux_x86_64,
    _stylua_linux_arm64,
    _stylua_macos_arm64,
    _stylua_windows_x86_64,
    _taplo_linux_x86_64,
    _taplo_linux_arm64,
    _taplo_macos_arm64,
    _taplo_windows_x86_64,
    _taplo_windows_arm64,
    _terraform_linux_x86_64,
    _terraform_linux_arm64,
    _terraform_macos_arm64,
    _terraform_windows_x86_64,
    _terraform_windows_arm64,
    _ty_linux_x86_64,
    _ty_linux_arm64,
    _ty_macos_arm64,
    _ty_windows_x86_64,
    _ty_windows_arm64,
    _latexindent_linux_x86_64,
    _latexindent_linux_arm64,
    _latexindent_macos_arm64,
    _latexindent_windows_x86_64,
    _typstyle_linux_x86_64,
    _typstyle_linux_arm64,
    _typstyle_macos_arm64,
    _typstyle_windows_x86_64,
    _typstyle_windows_arm64,
    _uv_linux_x86_64,
    _uv_linux_arm64,
    _uv_macos_arm64,
    _uv_windows_x86_64,
    _uv_windows_arm64,
    _vale_linux_x86_64,
    _vale_linux_arm64,
    _vale_macos_arm64,
    _vale_windows_x86_64,
    _vale_windows_arm64,
    _wasmtime_linux_x86_64,
    _wasmtime_linux_arm64,
    _wasmtime_macos_arm64,
    _wasmtime_windows_x86_64,
    _wasmtime_windows_arm64,
    _wasmtime_macos_x86_64,
    _yamlfmt_linux_x86_64,
    _yamlfmt_linux_arm64,
    _yamlfmt_macos_arm64,
    _yamlfmt_windows_x86_64,
    _yamlfmt_windows_arm64,
]

TOOL_ARTIFACTS = _ARTIFACTS

def _repo_name(artifact):
    return "dx_%s_%s" % (artifact["tool"], artifact_platform_key(artifact))

def _standalone_tool_repo_impl(ctx):
    acquire_tool(ctx)
    return ctx.repo_metadata(reproducible = True)

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
    return ctx.repo_metadata(reproducible = True)

_hub_repo = repository_rule(
    implementation = _hub_repo_impl,
    attrs = {
        "artifacts": attr.string(
            doc = "JSON map from tool name to platform repository names.",
            mandatory = True,
        ),
        "cpu_arm64": attr.string(
            doc = "Constraint value selecting arm64 CPUs for dx tools.",
            mandatory = True,
        ),
        "cpu_x86_64": attr.string(
            doc = "Constraint value selecting x86_64 CPUs for dx tools.",
            mandatory = True,
        ),
        "os_linux": attr.string(
            doc = "Constraint value selecting Linux for dx tools.",
            mandatory = True,
        ),
        "os_macos": attr.string(
            doc = "Constraint value selecting macOS for dx tools.",
            mandatory = True,
        ),
        "os_windows": attr.string(
            doc = "Constraint value selecting Windows for dx tools.",
            mandatory = True,
        ),
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
    "cpu_arm64": attr.label(
        doc = "Constraint value selecting arm64 CPUs for dx tools.",
        mandatory = True,
    ),
    "cpu_x86_64": attr.label(
        doc = "Constraint value selecting x86_64 CPUs for dx tools.",
        mandatory = True,
    ),
    "os_linux": attr.label(
        doc = "Constraint value selecting Linux for dx tools.",
        mandatory = True,
    ),
    "os_macos": attr.label(
        doc = "Constraint value selecting macOS for dx tools.",
        mandatory = True,
    ),
    "os_windows": attr.label(
        doc = "Constraint value selecting Windows for dx tools.",
        mandatory = True,
    ),
})

dx_tools = module_extension(
    implementation = _dx_tools_impl,
    tag_classes = {"platform": _platform},
)
