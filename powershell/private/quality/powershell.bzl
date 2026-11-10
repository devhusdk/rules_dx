"""PowerShell real quality aspects."""

load("@rules_powershell//powershell:defs.bzl", "PwshInfo")
load("//core/quality:pipeline.bzl", "real_request_tool_env")
load("//core/quality:real_core.bzl", "real_config", "real_context", "real_make_aspect", "real_make_probe_aspect")

TOOLS = {
    "psscriptanalyzer": {
        "attr": attr.label(default = "//powershell/private/quality:psscriptanalyzer_tool", cfg = "exec", executable = True),
        "env": [["DOTNET_CLI_TELEMETRY_OPTOUT", "1"], ["POWERSHELL_TELEMETRY_OPTOUT", "1"], ["POWERSHELL_UPDATECHECK", "Off"]],
        "kind": "executable",
        "resources": "heavy",
        "shard": "powershell",
    },
}

SHARD_SPECS = {
    "real_powershell_format": {
        "capability": "format",
        "doc": "Additive PowerShell format family aspect (PSScriptAnalyzer Invoke-Formatter).",
        "shard": "powershell",
        "suffix": "-powershell",
    },
    "real_powershell_lint": {
        "capability": "lint",
        "doc": "Additive PowerShell lint family aspect (PSScriptAnalyzer).",
        "shard": "powershell",
        "suffix": "-powershell",
    },
}

def _hook(target, ctx, _resolved, _capability, _what, _shared):
    context_files = list(target[DefaultInfo].files.to_list())
    upstream = getattr(ctx.rule.attr, "upstream", None)
    if type(upstream) == "Target" and PwshInfo in upstream:
        context_files.extend(upstream[DefaultInfo].default_runfiles.files.to_list())
    pairs = []
    module_roots = {}
    for f in context_files:
        ws_path = f.short_path
        if f.extension not in ["ps1", "psd1", "psm1"]:
            continue
        if ws_path.startswith("../"):
            if "_toolchains__" in ws_path.split("/")[1]:
                continue
            ws_path = "external/" + ws_path[3:]
        if ".." in ws_path.split("/"):
            continue
        if f.extension in ["psd1", "psm1"]:
            module_dir = ws_path.rpartition("/")[0]
            module_roots[module_dir.rpartition("/")[0] or "."] = True
        pairs.append((ws_path, f, False))
    env = [real_request_tool_env("DX_PWSH_MODULE_ROOTS", "|".join(sorted(module_roots.keys())))]
    return real_context(resolve_pairs = pairs, env = env)

HOOK_ORDER = ["psscriptanalyzer"]

HOOKS = {"psscriptanalyzer": _hook}

_CFG = real_config(
    tools = TOOLS,
    shard_specs = SHARD_SPECS,
    hook_order = HOOK_ORDER,
    context_hooks = HOOKS,
    source_recognizers = [],
    harness_sources = lambda _target, _capability, direct_sources: direct_sources,
    toolchain = None,
    check_policy = False,
)

real_powershell_format_aspect = real_make_aspect(_CFG, "real_powershell_format")

real_powershell_lint_aspect = real_make_aspect(_CFG, "real_powershell_lint")

PowershellShardProbeInfo = provider(
    doc = "Shard names the powershell shard probe found for one target and capability.",
    fields = {
        "capability": "lint, format or typecheck",
        "shards": "sorted shard names whose tool filter keeps a resolved stage",
    },
)

real_powershell_format_probe_aspect = real_make_probe_aspect(_CFG, "format", "powershell", PowershellShardProbeInfo)

real_powershell_lint_probe_aspect = real_make_probe_aspect(_CFG, "lint", "powershell", PowershellShardProbeInfo)
