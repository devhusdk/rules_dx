"""JavaScript and TypeScript real quality aspects."""

load("//core/quality:family_hooks.bzl", "default_files_hook", "without")
load("//core/quality/private:native_config_impl.bzl", "native_config_scratch_rel")
load("//core/quality:real_core.bzl", "real_config", "real_context", "real_make_aspect", "real_make_probe_aspect")
load("//javascript/private/quality:tsc.bzl", "TscInfo", "tsc_aspect")

TOOLS = {
    "eslint": {
        "attr": attr.label(default = "//javascript/private/quality/bin:eslint", cfg = "exec", executable = True),
        "env": [["JS_BINARY__NO_CD_BINDIR", "1"]],
        "extra": "files",
        "kind": "executable",
        "resources": "heavy",
        "shard": "js",
    },
    "gplint": {
        "attr": attr.label(default = "//javascript/private/quality/bin:gplint", cfg = "exec", executable = True),
        "env": [["JS_BINARY__NO_CD_BINDIR", "1"]],
        "extra": "files",
        "kind": "executable",
        "resources": "heavy",
        "shard": "js",
    },
    "graphql_eslint": {
        "attr": attr.label(default = "//javascript/private/quality/bin:graphql_eslint", cfg = "exec", executable = True),
        "env": [["JS_BINARY__NO_CD_BINDIR", "1"]],
        "extra": "files",
        "kind": "executable",
        "resources": "heavy",
        "shard": "js",
    },
    "prettier": {
        "attr": attr.label(default = "//javascript/private/quality/bin:prettier", cfg = "exec", executable = True),
        "env": [["JS_BINARY__NO_CD_BINDIR", "1"]],
        "extra": "files",
        "kind": "executable",
        "resources": "heavy",
        "shard": "js",
    },
    "redocly": {
        "attr": attr.label(default = "//javascript/private/quality/bin:redocly", cfg = "exec", executable = True),
        "env": [["JS_BINARY__NO_CD_BINDIR", "1"]],
        "extra": "files",
        "kind": "executable",
        "resources": "medium",
        "shard": "js",
    },
    "remark": {
        "attr": attr.label(default = "//javascript/private/quality/bin:remark", cfg = "exec", executable = True),
        "env": [["JS_BINARY__NO_CD_BINDIR", "1"]],
        "extra": "files",
        "kind": "executable",
        "resources": "heavy",
        "shard": "js",
    },
    "stylelint": {
        "attr": attr.label(default = "//javascript/private/quality/bin:stylelint", cfg = "exec", executable = True),
        "env": [["JS_BINARY__NO_CD_BINDIR", "1"]],
        "extra": "files",
        "kind": "executable",
        "resources": "heavy",
        "shard": "js",
    },
    "tsc": {
        "kind": "upstream",
        "resources": "heavy",
        "shard": "js",
    },
}

SHARD_SPECS = {
    "real_js_format": {
        "capability": "format",
        "doc": "Additive JavaScript/JSON/MDX format family aspect (Prettier, remark).",
        "shard": "js",
        "suffix": "-js",
    },
    "real_js_lint": {
        "capability": "lint",
        "doc": "Additive JavaScript lint family aspect (ESLint/Stylelint/graphql-eslint/Redocly opt-in).",
        "requires": [tsc_aspect],
        "shard": "js",
        "suffix": "-js",
    },
    "real_js_typecheck": {
        "capability": "typecheck",
        "doc": "Additive TypeScript typecheck family aspect (delegated tsc over typescript_library and js_library).",
        "requires": [tsc_aspect],
        "shard": "js",
        "suffix": "-js",
    },
}

def _tsc_hook(target, _ctx, resolved, _capability, _what, _shared):
    if TscInfo not in target:
        return without(resolved, "tsc")
    info = target[TscInfo]
    return real_context(upstream = [info.stdout], late_tool_files = [("dx-tsc/exit", info.status)], late_inputs = [info.stdout, info.status])

def _eslint_hook(target, _ctx, _resolved, _capability, _what, shared):
    if TscInfo not in target or target[TscInfo].tsconfig == None:
        return real_context()
    staged = {}
    for f in target[TscInfo].inputs.to_list():
        rel = native_config_scratch_rel(f.short_path)
        if rel != None and rel not in shared.path_to_file:
            staged[rel] = f
    return real_context(
        late_tool_files = sorted(staged.items()),
        late_inputs = staged.values(),
        env = [["DX_ESLINT_TYPED", "1"]],
    )

HOOK_ORDER = ["eslint", "tsc", "redocly"]

HOOKS = {
    "eslint": _eslint_hook,
    "tsc": _tsc_hook,
    "redocly": default_files_hook(["yaml", "yml", "json"]),
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

real_js_format_aspect = real_make_aspect(_CFG, "real_js_format")

real_js_lint_aspect = real_make_aspect(_CFG, "real_js_lint")

real_js_typecheck_aspect = real_make_aspect(_CFG, "real_js_typecheck")

JsShardProbeInfo = provider(
    doc = "Shard names the js shard probe found for one target and capability.",
    fields = {
        "capability": "lint, format or typecheck",
        "shards": "sorted shard names whose tool filter keeps a resolved stage",
    },
)

real_js_format_probe_aspect = real_make_probe_aspect(_CFG, "format", "js", JsShardProbeInfo)

real_js_lint_probe_aspect = real_make_probe_aspect(_CFG, "lint", "js", JsShardProbeInfo)

real_js_typecheck_probe_aspect = real_make_probe_aspect(_CFG, "typecheck", "js", JsShardProbeInfo)
