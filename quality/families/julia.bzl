"""Julia real quality aspects."""

load("//libs/starlark:rlocation.bzl", "rlocation_path")
load("@rules_julia//julia:julia_info.bzl", "JuliaInfo")
load("//quality:family_hooks.bzl", "stage", "without")
load("//quality:real_core.bzl", "real_config", "real_context", "real_make_aspect", "real_make_probe_aspect")

TOOLS = {
    "jet": {
        "attr": attr.label(default = "//config:tool_jet", allow_files = True, cfg = "exec"),
        "fallback": "=@rules_dx//julia/tools/jet:jet",
        "kind": "setting",
        "resources": "heavy",
        "shard": "julia",
    },
    "julialint": {
        "attr": attr.label(default = "//config:tool_julialint", allow_files = True, cfg = "exec"),
        "fallback": "=@rules_dx//julia/tools/lint:julialint",
        "kind": "setting",
        "resources": "medium",
        "shard": "julia",
    },
    "julia_format": {
        "attr": attr.label(default = "//config:tool_julia_format", allow_files = True, cfg = "exec"),
        "kind": "setting",
        "resources": "medium",
        "shard": "julia",
        "target_tool": True,
    },
}

SHARD_SPECS = {
    "real_julia_typecheck": {
        "capability": "typecheck",
        "doc": "Additive Julia typecheck family aspect (JET).",
        "shard": "julia",
        "suffix": "-julia",
    },
    "real_julia_format": {
        "capability": "format",
        "doc": "Additive Julia format family aspect (JuliaFormatter).",
        "shard": "julia",
        "suffix": "-julia",
    },
    "real_julia_lint": {
        "capability": "lint",
        "doc": "Additive Julia lint family aspect (JuliaWorkspaces).",
        "shard": "julia",
        "suffix": "-julia",
    },
}


def _jet_staging(target, ctx, path_to_file):
    """Returns the staged dependency sources and the analysis config of one Julia target."""
    info = target[JuliaInfo]
    owned = {f.path: ws for (ws, f) in path_to_file.items()}
    main = getattr(ctx.rule.attr, "main", None)
    main_files = main.files.to_list() if main != None else []
    if info.entry != None and info.entry.path in owned:
        roots = [owned[info.entry.path]]
    elif len(main_files) == 1 and main_files[0].path in owned:
        roots = [owned[main_files[0].path]]
    else:
        roots = sorted(owned.values())
    staged = []
    seen = {}
    candidates = info.transitive_srcs.to_list() + [f for f in info.runfiles.files.to_list() if f.basename in ("Project.toml", "JuliaProject.toml") and "/share/julia/" not in f.short_path]
    for f in sorted(candidates, key = lambda f: f.path):
        if f.is_directory or f.path in seen:
            continue
        seen[f.path] = True
        staged.append(("dx-jet/rf/" + rlocation_path(ctx, f), f))
    config = ctx.actions.declare_file(target.label.name + "-jet-config.toml")
    ctx.actions.write(
        output = config,
        content = "includes = %s\nroots = %s\n" % (
            json.encode(sorted(info.includes.to_list())),
            json.encode(roots),
        ),
    )
    return (staged, config)

def _jet_hook(target, ctx, resolved, _capability, _what, shared):
    if JuliaInfo not in target:
        return without(resolved, "jet")
    (staged, config) = _jet_staging(target, ctx, shared.path_to_file)
    tool_files = []
    inputs = []
    if config != None:
        for (rel, staged_file) in staged:
            stage(tool_files, inputs, rel, staged_file)
        stage(tool_files, inputs, "dx-jet/config.toml", config)
    return real_context(tool_files = tool_files, inputs = inputs)


HOOK_ORDER = ["jet"]

HOOKS = {"jet": _jet_hook}

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

real_julia_format_aspect = real_make_aspect(_CFG, "real_julia_format")

real_julia_lint_aspect = real_make_aspect(_CFG, "real_julia_lint")

real_julia_typecheck_aspect = real_make_aspect(_CFG, "real_julia_typecheck")

JuliaShardProbeInfo = provider(
    doc = "Shard names the julia shard probe found for one target and capability.",
    fields = {
        "capability": "lint, format or typecheck",
        "shards": "sorted shard names whose tool filter keeps a resolved stage",
    },
)

real_julia_format_probe_aspect = real_make_probe_aspect(_CFG, "format", "julia", JuliaShardProbeInfo)

real_julia_lint_probe_aspect = real_make_probe_aspect(_CFG, "lint", "julia", JuliaShardProbeInfo)

real_julia_typecheck_probe_aspect = real_make_probe_aspect(_CFG, "typecheck", "julia", JuliaShardProbeInfo)
