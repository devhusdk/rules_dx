"""C# and F# real quality aspects."""

load("@bazel_skylib//rules:common_settings.bzl", "BuildSettingInfo")
load("@rules_dotnet//dotnet/private:providers.bzl", "DotnetAssemblyRuntimeInfo")
load("//dotnet:symbols.bzl", "fsharp_compile_defines")
load("//quality:family_hooks.bzl", "stage")
load("//quality:fsc.bzl", "FscInfo", "FscUnsupportedInfo", "fsc_aspect")
load("//quality:real_core.bzl", "real_config", "real_context", "real_make_aspect")
load("//quality:roslyn.bzl", "RoslynInfo", "roslyn_aspect")

TOOLS = {
    "csharpier": {
        "attr": attr.label(default = "//quality/tools/dotnet:csharpier", cfg = "exec", executable = True),
        "kind": "executable",
        "resources": "heavy",
        "shard": "dotnet",
    },
    "fantomas": {
        "attr": attr.label(default = "//quality/tools/dotnet:fantomas", cfg = "exec", executable = True),
        "kind": "executable",
        "resources": "heavy",
        "shard": "dotnet",
    },
    "fsharplint": {
        "attr": attr.label(default = "//quality/tools/dotnet/fsharplint", cfg = "exec", executable = True),
        "kind": "executable",
        "resources": "heavy",
        "shard": "dotnet",
    },
    "csc": {
        "kind": "upstream",
        "resources": "heavy",
        "shard": "dotnet",
    },
    "fsc": {
        "kind": "upstream",
        "resources": "heavy",
        "shard": "dotnet",
    },
    "roslyn": {
        "kind": "upstream",
        "resources": "heavy",
        "shard": "dotnet",
    },
}

SHARD_SPECS = {
    "real_dotnet_format": {
        "capability": "format",
        "doc": "Additive C#/F# format family aspect (CSharpier/Fantomas).",
        "shard": "dotnet",
        "suffix": "-dotnet",
    },
    "real_dotnet_lint": {
        "capability": "lint",
        "doc": "Additive C#/F# lint family aspect (Roslyn/FSharpLint).",
        "requires": [roslyn_aspect],
        "shard": "dotnet",
        "suffix": "-dotnet",
    },
    "real_dotnet_typecheck": {
        "capability": "typecheck",
        "doc": "Additive C#/F# typecheck family aspect (csc and fsc compiler diagnostics).",
        "requires": [fsc_aspect, roslyn_aspect],
        "shard": "dotnet",
        "suffix": "-dotnet",
    },
}

def _fsharplint_context(target, ctx, what):
    """Returns the dependency assemblies and the compile context file for one F# target."""
    if DotnetAssemblyRuntimeInfo not in target:
        fail(what + ": fsharplint needs the compiled dependency assemblies of this target")
    if not (hasattr(ctx.rule.attr, "defines") and hasattr(ctx.rule.attr, "target_frameworks") and hasattr(ctx.rule.attr, "_target_framework")):
        fail(what + ": fsharplint needs an fsharp_* target with compile symbols")
    runtime = target[DotnetAssemblyRuntimeInfo]
    seen = {lib.basename: True for lib in runtime.libs}
    refs = []
    for info in runtime.deps.to_list():
        for lib in info.libs:
            if lib.basename.endswith(".dll") and lib.basename not in seen:
                seen[lib.basename] = True
                refs.append(lib)
    context = ctx.actions.declare_file(target.label.name + "-fsharplint-context.json")
    ctx.actions.write(
        output = context,
        content = json.encode({
            "defines": fsharp_compile_defines(ctx.rule.attr._target_framework[BuildSettingInfo].value, ctx.rule.attr.target_frameworks, ctx.rule.attr.defines),
            "langversion": ctx.rule.attr.langversion,
            "refs": ["dx-fsharplint/ref/%d-%s" % (index, lib.basename) for index, lib in enumerate(refs)],
            "sources": [f.short_path for f in ctx.rule.files.srcs],
        }),
    )
    return (refs, context)

def _fsharplint_hook(target, ctx, _resolved, _capability, what, _shared):
    (refs, context) = _fsharplint_context(target, ctx, what)
    tool_files = []
    inputs = []
    for index, lib in enumerate(refs):
        stage(tool_files, inputs, "dx-fsharplint/ref/%d-%s" % (index, lib.basename), lib)
    stage(tool_files, inputs, "dx-fsharplint/context.json", context)
    return real_context(tool_files = tool_files, inputs = inputs)

def _roslyn_hook(target, _ctx, _resolved, _capability, what, _shared):
    if RoslynInfo not in target:
        fail(what + ": roslyn and csc need a csharp_* target compiled with the SDK analyzers")
    info = target[RoslynInfo]
    tool_files = []
    inputs = []
    for i in range(len(info.tfms)):
        inputs.append(info.sarifs[i])
        stage(tool_files, inputs, "dx-roslyn/" + info.tfms[i] + "/exit", info.statuses[i])
    return real_context(upstream = info.sarifs, late_tool_files = tool_files, late_inputs = inputs)

def _fsc_hook(target, _ctx, _resolved, _capability, what, _shared):
    if FscUnsupportedInfo in target:
        fail(what + ": fsc is unavailable: " + target[FscUnsupportedInfo].reason)
    if FscInfo not in target:
        fail(what + ": fsc needs an fsharp_* target")
    info = target[FscInfo]
    tool_files = []
    inputs = []
    for i in range(len(info.tfms)):
        inputs.append(info.logs[i])
        stage(tool_files, inputs, "dx-fsc/" + info.tfms[i] + "/exit", info.statuses[i])
    return real_context(upstream = info.logs, late_tool_files = tool_files, late_inputs = inputs)

HOOK_ORDER = ["fsharplint", "roslyn", "csc", "fsc"]

HOOKS = {
    "csc": _roslyn_hook,
    "fsc": _fsc_hook,
    "fsharplint": _fsharplint_hook,
    "roslyn": _roslyn_hook,
}

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

real_dotnet_format_aspect = real_make_aspect(_CFG, "real_dotnet_format")

real_dotnet_lint_aspect = real_make_aspect(_CFG, "real_dotnet_lint")

real_dotnet_typecheck_aspect = real_make_aspect(_CFG, "real_dotnet_typecheck")
