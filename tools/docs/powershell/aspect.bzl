"""PowerShell documentation shard aspect."""

load("//tools/docs/ir:shard.bzl", "DocsShardInfo")
load("//tools/docs/powershell:defs.bzl", "powershell_shard_action")
load("@rules_powershell//powershell:defs.bzl", _PwshInfo = "PwshInfo")

def _powershell_docs_shard(target, ctx):
    if _PwshInfo not in target or DocsShardInfo in target:
        return None
    if ctx.rule.kind.endswith("_test") or ctx.rule.kind.endswith("_binary"):
        return None
    files = sorted(target[_PwshInfo].srcs.to_list(), key = lambda f: f.path)
    sources = [f for f in files if f.extension in ("ps1", "psm1")]
    manifests = [f for f in files if f.extension == "psd1"]
    if not sources or len(manifests) > 1:
        return None
    package = manifests[0].basename[:-len(".psd1")] if manifests else ctx.label.name
    pwsh = ctx.attr._pwsh[platform_common.ToolchainInfo]
    return powershell_shard_action(ctx, sources, manifests[0] if manifests else None, package, "", "_dx_docs/" + ctx.label.name, pwsh, ctx.file._pwsh_script, ctx.executable._pwsh_extract).shard

def _real_powershell_docs_aspect_impl(target, ctx):
    shard = _powershell_docs_shard(target, ctx)
    if not shard:
        return []
    return [OutputGroupInfo(dx_results = depset([shard]), dx_docs_shards = depset([shard]))]

real_powershell_docs_aspect = aspect(
    implementation = _real_powershell_docs_aspect_impl,
    doc = "Puts the PowerShell docs shard of a target in dx_results and dx_docs_shards.",
    attrs = {
        "_pwsh": attr.label(
            cfg = "exec",
            default = Label("@rules_powershell//powershell:current_pwsh_toolchain"),
        ),
        "_pwsh_extract": attr.label(
            cfg = "exec",
            default = Label("//tools/docs/powershell/extract:powershell_extract"),
            executable = True,
        ),
        "_pwsh_script": attr.label(
            allow_single_file = True,
            default = Label("//tools/docs/powershell:extract.ps1"),
        ),
    },
    fragments = ["cpp"],
)
