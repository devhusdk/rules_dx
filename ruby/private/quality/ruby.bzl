"""Ruby real quality aspects."""

load("@rules_ruby//ruby/private:providers.bzl", "RubyFilesInfo")
load("//core:rlocation.bzl", "external_path")
load("//core/quality:real_core.bzl", "real_config", "real_context", "real_make_aspect", "real_make_probe_aspect")

TOOLS = {
    "sorbet": {
        "attr": attr.label(default = "//config:tool_sorbet", allow_files = True, cfg = "exec"),
        "fallback": "=@rules_dx//ruby/private/tools:sorbet",
        "kind": "setting",
        "resources": "heavy",
        "shard": "ruby",
    },
    "rubocop": {
        "attr": attr.label(default = "//config:tool_rubocop", cfg = "exec", executable = True),
        "kind": "executable",
        "resources": "medium",
        "shard": "ruby",
    },
}

SHARD_SPECS = {
    "real_ruby_typecheck": {
        "capability": "typecheck",
        "doc": "Additive Ruby typecheck family aspect (Sorbet).",
        "shard": "ruby",
    },
    "real_ruby_format": {
        "capability": "format",
        "doc": "Additive Ruby format family aspect (RuboCop).",
        "shard": "ruby",
    },
    "real_ruby_lint": {
        "capability": "lint",
        "doc": "Additive Ruby lint family aspect (RuboCop).",
        "shard": "ruby",
    },
}

def _sorbet_dep_line(f):
    if not f.path.endswith((".rb", ".rbi")):
        return None
    return "dx-sorbet/dep/" + external_path(f.short_path) + "\t" + f.path

def _sorbet_hook(target, _ctx, _resolved, _capability, _what, _shared):
    if RubyFilesInfo not in target:
        return real_context()
    return real_context(dep_lists = [(target[RubyFilesInfo].transitive_srcs, _sorbet_dep_line)])

HOOK_ORDER = ["sorbet"]

HOOKS = {"sorbet": _sorbet_hook}

_CFG = real_config(
    tools = TOOLS,
    shard_specs = SHARD_SPECS,
    hook_order = HOOK_ORDER,
    context_hooks = HOOKS,
)

real_ruby_format_aspect = real_make_aspect(_CFG, "real_ruby_format")

real_ruby_lint_aspect = real_make_aspect(_CFG, "real_ruby_lint")

real_ruby_typecheck_aspect = real_make_aspect(_CFG, "real_ruby_typecheck")

RubyShardProbeInfo = provider(
    doc = "Shard names the ruby shard probe found for one target and capability.",
    fields = {
        "capability": "lint, format or typecheck",
        "shards": "sorted shard names whose tool filter keeps a resolved stage",
    },
)

QUALITY_CFG = _CFG

real_ruby_format_probe_aspect = real_make_probe_aspect(_CFG, "format", "ruby", RubyShardProbeInfo)

real_ruby_lint_probe_aspect = real_make_probe_aspect(_CFG, "lint", "ruby", RubyShardProbeInfo)

real_ruby_typecheck_probe_aspect = real_make_probe_aspect(_CFG, "typecheck", "ruby", RubyShardProbeInfo)
