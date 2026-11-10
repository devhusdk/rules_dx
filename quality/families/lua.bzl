"""Lua real quality aspects."""

load("//lua/private:providers.bzl", "LuaInfo")
load("//quality:family_hooks.bzl", "stage")
load("//quality:real_core.bzl", "real_config", "real_context", "real_make_aspect", "real_make_probe_aspect")

TOOLS = {
    "luals": {
        "attr": attr.label(default = "//config:tool_luals", allow_files = True, cfg = "exec"),
        "fallback": "=@rules_dx//lua/private/tools:luals",
        "kind": "setting",
        "resources": "heavy",
        "shard": "lua",
    },
    "selene": {
        "attr": attr.label(default = "//quality/tools/rust:selene_tool", cfg = "exec", executable = True),
        "kind": "executable",
        "resources": "light",
        "shard": "lua",
    },
    "stylua": {
        "attr": attr.label(default = "//config:tool_stylua", allow_single_file = True, cfg = "exec"),
        "kind": "file",
        "resources": "light",
        "shard": "lua",
    },
}

SHARD_SPECS = {
    "real_lua_typecheck": {
        "capability": "typecheck",
        "doc": "Additive Lua typecheck family aspect (lua-language-server).",
        "shard": "lua",
        "suffix": "-lua",
    },
    "real_lua_lint": {
        "capability": "lint",
        "doc": "Additive Lua lint family aspect (selene).",
        "shard": "lua",
        "suffix": "-lua",
    },
    "real_lua_format": {
        "capability": "format",
        "doc": "Additive Lua format family aspect (stylua).",
        "shard": "lua",
        "suffix": "-lua",
    },
}

def _luals_hook(target, ctx, _resolved, _capability, _what, shared):
    # Dependency modules become the library, one file per module name; the owned sources
    # are checked in place, so they stay out of the library. Libraries carry LuaInfo; a
    # lua_binary or lua_test has none, so its deps provide the modules.
    owned_paths = {f.path: True for f in shared.path_to_file.values()}
    infos = [target[LuaInfo]] if LuaInfo in target else [dep[LuaInfo] for dep in getattr(ctx.rule.attr, "deps", []) if LuaInfo in dep]
    tool_files = []
    inputs = []
    seen_modules = {}
    for info in infos:
        for (module, module_file) in sorted(info.modules.to_list(), key = lambda m: m[0]):
            if module_file.path not in owned_paths and module not in seen_modules:
                seen_modules[module] = True
                stage(tool_files, inputs, "dx-luals/mod/" + module.replace(".", "/") + ".lua", module_file)
    return real_context(tool_files = tool_files, inputs = inputs)

HOOK_ORDER = ["luals"]

HOOKS = {"luals": _luals_hook}

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

real_lua_typecheck_aspect = real_make_aspect(_CFG, "real_lua_typecheck")

real_lua_lint_aspect = real_make_aspect(_CFG, "real_lua_lint")

real_lua_format_aspect = real_make_aspect(_CFG, "real_lua_format")

LuaShardProbeInfo = provider(
    doc = "Shard names the lua shard probe found for one target and capability.",
    fields = {
        "capability": "lint, format or typecheck",
        "shards": "sorted shard names whose tool filter keeps a resolved stage",
    },
)

real_lua_format_probe_aspect = real_make_probe_aspect(_CFG, "format", "lua", LuaShardProbeInfo)

real_lua_lint_probe_aspect = real_make_probe_aspect(_CFG, "lint", "lua", LuaShardProbeInfo)

real_lua_typecheck_probe_aspect = real_make_probe_aspect(_CFG, "typecheck", "lua", LuaShardProbeInfo)
