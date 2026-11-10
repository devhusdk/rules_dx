"""Erlang and Elixir real quality aspects."""

load("//erlang/private:providers.bzl", "BeamInfo", "DialyzerToolInfo")
load("//quality:elixirc.bzl", "ElixircInfo", "elixirc_aspect")
load("//quality:erlc.bzl", "ErlcInfo", "erlc_aspect")
load("//quality:family_hooks.bzl", "stage", "without")
load("//quality:real_core.bzl", "real_config", "real_context", "real_make_aspect", "real_make_probe_aspect")

TOOLS = {
    "credo": {
        "attr": attr.label(default = "//config:tool_credo", allow_files = True, cfg = "exec"),
        "fallback": "=@rules_dx//elixir/private/tools:credo",
        "kind": "setting",
        "resources": "medium",
        "shard": "elixir",
    },
    "dialyzer": {
        "attr": attr.label(default = "//config:tool_dialyzer", allow_files = True, cfg = "exec"),
        "fallback": "=@rules_dx//erlang/private/tools:dialyzer",
        "kind": "setting",
        "resources": "heavy",
        "shard": "erlang",
    },
    "elixirc": {
        "kind": "upstream",
        "resources": "medium",
        "shard": "erlang",
    },
    "elvis": {
        "attr": attr.label(default = "//config:tool_elvis", allow_files = True, cfg = "exec"),
        "fallback": "=@rules_dx//erlang/private/tools:elvis",
        "kind": "setting",
        "resources": "medium",
        "shard": "erlang",
    },
    "erlc": {
        "kind": "upstream",
        "resources": "medium",
        "shard": "erlang",
    },
    "erlfmt": {
        "attr": attr.label(default = "//config:tool_erlfmt", allow_files = True, cfg = "exec"),
        "fallback": "=@rules_dx//erlang/private/tools:erlfmt",
        "kind": "setting",
        "resources": "medium",
        "shard": "erlang",
    },
    "mix_format": {
        "attr": attr.label(default = "//config:tool_mix_format", allow_files = True, cfg = "exec"),
        "fallback": "=@rules_dx//elixir/private/tools:mix_format",
        "kind": "setting",
        "resources": "medium",
        "shard": "elixir",
    },
}

SHARD_SPECS = {
    "real_erlang_format": {
        "capability": "format",
        "doc": "Additive Erlang format family aspect (erlfmt).",
        "shard": "erlang",
        "suffix": "-erlang",
    },
    "real_erlang_lint": {
        "capability": "lint",
        "doc": "Additive Erlang lint family aspect (elvis).",
        "shard": "erlang",
        "suffix": "-erlang",
    },
    "real_erlang_typecheck": {
        "capability": "typecheck",
        "doc": "Additive Erlang typecheck family aspect (dialyzer, erlc and elixirc warnings).",
        "requires": [erlc_aspect, elixirc_aspect],
        "shard": "erlang",
        "suffix": "-erlang",
    },
    "real_elixir_format": {
        "capability": "format",
        "doc": "Additive Elixir format family aspect (mix format).",
        "shard": "elixir",
        "suffix": "-elixir",
    },
    "real_elixir_lint": {
        "capability": "lint",
        "doc": "Additive Elixir lint family aspect (credo).",
        "shard": "elixir",
        "suffix": "-elixir",
    },
}

def _stage_beams(target, prefix, tool_files, inputs):
    """Stages the owned beams, the compiler's own beams and the other dependency beams."""
    code = sorted(target[BeamInfo].code, key = lambda f: f.basename)
    owned = sorted(target[BeamInfo].owned.to_list(), key = lambda f: f.short_path)
    owned_paths = {f.path: True for f in owned}
    aux = [f for f in sorted(target[BeamInfo].deps.to_list(), key = lambda f: f.short_path) if f.path not in owned_paths]
    for index, beam in enumerate(owned):
        stage(tool_files, inputs, "%s/owned/%d/%s" % (prefix, index, beam.basename), beam)
    for beam in code:
        stage(tool_files, inputs, prefix + "/pa/" + beam.basename, beam)
    seen_aux = {}
    for index, beam in enumerate(aux):
        if beam.path in seen_aux:
            continue
        seen_aux[beam.path] = True
        stage(tool_files, inputs, "%s/aux/%d/%s" % (prefix, index, beam.basename), beam)

def _dialyzer_hook(target, ctx, resolved, _capability, what, _shared):
    if BeamInfo not in target or len(target[BeamInfo].owned.to_list()) == 0:
        return without(resolved, "dialyzer")
    tool_target = getattr(ctx.attr, "_dialyzer")
    if DialyzerToolInfo not in tool_target:
        fail(what + ": dialyzer needs PLTs; set --@rules_dx//config:tool_dialyzer=@rules_dx//erlang/private/tools:dialyzer")

    # The tool's PLTs come first; a language's own follow, without repeating one.
    plts = []
    seen_plts = {}
    for plt in tool_target[DialyzerToolInfo].plts + target[BeamInfo].plts:
        if plt.path not in seen_plts:
            seen_plts[plt.path] = True
            plts.append(plt)
    tool_files = []
    inputs = []
    for index, plt in enumerate(plts):
        stage(tool_files, inputs, "dx-dialyzer/plt/%s-%s" % (("000" + str(index))[-3:], plt.basename), plt)
    _stage_beams(target, "dx-dialyzer", tool_files, inputs)
    return real_context(tool_files = tool_files, inputs = inputs)

def _erlc_hook(target, _ctx, resolved, _capability, _what, _shared):
    if ErlcInfo not in target:
        return without(resolved, "erlc")
    info = target[ErlcInfo]
    tool_files = []
    inputs = []
    for i in range(len(info.names)):
        inputs.append(info.logs[i])
        stage(tool_files, inputs, "dx-erlc/" + info.names[i] + "/exit", info.statuses[i])
    return real_context(upstream = info.logs, late_tool_files = tool_files, late_inputs = inputs)

def _elixirc_hook(target, _ctx, resolved, _capability, _what, _shared):
    if ElixircInfo not in target:
        return without(resolved, "elixirc")
    info = target[ElixircInfo]
    tool_files = []
    inputs = []
    for i in range(len(info.names)):
        inputs.append(info.logs[i])
        stage(tool_files, inputs, "dx-elixirc/" + info.names[i] + "/exit", info.statuses[i])
    return real_context(upstream = info.logs, late_tool_files = tool_files, late_inputs = inputs)

HOOK_ORDER = ["dialyzer", "erlc", "elixirc"]

HOOKS = {"dialyzer": _dialyzer_hook, "erlc": _erlc_hook, "elixirc": _elixirc_hook}

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

real_elixir_format_aspect = real_make_aspect(_CFG, "real_elixir_format")

real_elixir_lint_aspect = real_make_aspect(_CFG, "real_elixir_lint")

real_erlang_format_aspect = real_make_aspect(_CFG, "real_erlang_format")

real_erlang_lint_aspect = real_make_aspect(_CFG, "real_erlang_lint")

real_erlang_typecheck_aspect = real_make_aspect(_CFG, "real_erlang_typecheck")

BeamShardProbeInfo = provider(
    doc = "Shard names the beam shard probe found for one target and capability.",
    fields = {
        "capability": "lint, format or typecheck",
        "shards": "sorted shard names whose tool filter keeps a resolved stage",
    },
)

real_beam_format_probe_aspect = real_make_probe_aspect(_CFG, "format", "beam", BeamShardProbeInfo)

real_beam_lint_probe_aspect = real_make_probe_aspect(_CFG, "lint", "beam", BeamShardProbeInfo)

real_beam_typecheck_probe_aspect = real_make_probe_aspect(_CFG, "typecheck", "beam", BeamShardProbeInfo)
