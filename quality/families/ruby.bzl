"""Ruby real quality aspects."""

load("@rules_ruby//ruby/private:providers.bzl", "RubyFilesInfo")
load("//php/private:php.bzl", "staged_path")
load("//quality:family_hooks.bzl", "stage")
load("//quality:real_core.bzl", "real_config", "real_context", "real_make_aspect", "real_make_probe_aspect")

TOOLS = {
    "sorbet": {
        "attr": attr.label(default = "//config:tool_sorbet", allow_files = True, cfg = "exec"),
        "fallback": "=@rules_dx//ruby/private/tools:sorbet",
        "kind": "setting",
        "resources": "heavy",
        "shard": "ruby",
    },
    "rubocop": {
        "attr": attr.label(default = "//ruby/private/quality:rubocop", cfg = "exec", executable = True),
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
        "suffix": "-ruby",
    },
    "real_ruby_format": {
        "capability": "format",
        "doc": "Additive Ruby format family aspect (RuboCop).",
        "shard": "ruby",
        "suffix": "-ruby",
    },
    "real_ruby_lint": {
        "capability": "lint",
        "doc": "Additive Ruby lint family aspect (RuboCop).",
        "shard": "ruby",
        "suffix": "-ruby",
    },
}

def _sorbet_hook(target, _ctx, _resolved, _capability, _what, shared):
    # Dependency sources are only read for the constants they define; the owned sources
    # are the CLI paths, so they stay out of the dependency set. A target without
    # RubyFilesInfo has no dependencies to stage.
    owned_paths = {f.path: True for f in shared.path_to_file.values()}
    tool_files = []
    inputs = []
    if RubyFilesInfo in target:
        seen_deps = {}
        for dep_file in sorted(target[RubyFilesInfo].transitive_srcs.to_list(), key = lambda f: f.path):
            if dep_file.path.endswith((".rb", ".rbi")) and dep_file.path not in owned_paths and dep_file.path not in seen_deps:
                seen_deps[dep_file.path] = True
                stage(tool_files, inputs, "dx-sorbet/dep/" + staged_path(dep_file), dep_file)
    return real_context(tool_files = tool_files, inputs = inputs)

HOOK_ORDER = ["sorbet"]

HOOKS = {"sorbet": _sorbet_hook}

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

real_ruby_format_probe_aspect = real_make_probe_aspect(_CFG, "format", "ruby", RubyShardProbeInfo)

real_ruby_lint_probe_aspect = real_make_probe_aspect(_CFG, "lint", "ruby", RubyShardProbeInfo)

real_ruby_typecheck_probe_aspect = real_make_probe_aspect(_CFG, "typecheck", "ruby", RubyShardProbeInfo)
