"""JavaScript and TypeScript real quality aspects."""

load("//core/quality:family_hooks.bzl", "default_files_hook", "without")
load("//core/quality:real_core.bzl", "real_config", "real_context", "real_make_aspect", "real_make_probe_aspect")
load("//typescript/private/quality:tsc.bzl", "TscInfo", "tsc_aspect")

TOOLS = {
    "eslint": {
        "attr": attr.label(default = "//config:tool_eslint", cfg = "exec", executable = True),
        "env": [["JS_BINARY__NO_CD_BINDIR", "1"]],
        "extra": "files",
        "kind": "executable",
        "resources": "heavy",
        "shard": "eslint",
    },
    "gplint": {
        "attr": attr.label(default = "//config:tool_gplint", cfg = "exec", executable = True),
        "env": [["JS_BINARY__NO_CD_BINDIR", "1"]],
        "extra": "files",
        "kind": "executable",
        "resources": "heavy",
        "shard": "gplint",
    },
    "graphql_eslint": {
        "attr": attr.label(default = "//config:tool_graphql_eslint", cfg = "exec", executable = True),
        "env": [["JS_BINARY__NO_CD_BINDIR", "1"]],
        "extra": "files",
        "kind": "executable",
        "resources": "heavy",
        "shard": "graphql_eslint",
    },
    "prettier": {
        "attr": attr.label(default = "//config:tool_prettier", cfg = "exec", executable = True),
        "env": [["JS_BINARY__NO_CD_BINDIR", "1"]],
        "extra": "files",
        "kind": "executable",
        "resources": "heavy",
        "shard": "prettier",
    },
    "redocly": {
        "attr": attr.label(default = "//config:tool_redocly", cfg = "exec", executable = True),
        "env": [["JS_BINARY__NO_CD_BINDIR", "1"]],
        "extra": "files",
        "kind": "executable",
        "resources": "medium",
        "shard": "redocly",
    },
    "remark": {
        "attr": attr.label(default = "//config:tool_remark", cfg = "exec", executable = True),
        "env": [["JS_BINARY__NO_CD_BINDIR", "1"]],
        "extra": "files",
        "kind": "executable",
        "resources": "heavy",
        "shard": "remark",
    },
    "stylelint": {
        "attr": attr.label(default = "//config:tool_stylelint", cfg = "exec", executable = True),
        "env": [["JS_BINARY__NO_CD_BINDIR", "1"]],
        "extra": "files",
        "kind": "executable",
        "resources": "heavy",
        "shard": "stylelint",
    },
    "tsc": {
        "kind": "upstream",
        "resources": "heavy",
        "shard": "js",
    },
}

SHARD_SPECS = {
    "real_js_eslint_lint": {
        "capability": "lint",
        "doc": "Additive ESLint lint aspect.",
        "requires": [tsc_aspect],
        "shard": "eslint",
    },
    "real_js_gplint_lint": {
        "capability": "lint",
        "doc": "Additive gplint Gherkin lint aspect.",
        "shard": "gplint",
    },
    "real_js_graphql_eslint_lint": {
        "capability": "lint",
        "doc": "Additive graphql-eslint lint aspect.",
        "shard": "graphql_eslint",
    },
    "real_js_graphql_eslint_typecheck": {
        "capability": "typecheck",
        "doc": "Additive graphql-eslint schema typecheck aspect.",
        "shard": "graphql_eslint",
    },
    "real_js_prettier_format": {
        "capability": "format",
        "doc": "Additive Prettier format aspect.",
        "shard": "prettier",
    },
    "real_js_redocly_lint": {
        "capability": "lint",
        "doc": "Additive Redocly lint aspect.",
        "shard": "redocly",
    },
    "real_js_remark_format": {
        "capability": "format",
        "doc": "Additive remark MDX format aspect.",
        "shard": "remark",
    },
    "real_js_stylelint_lint": {
        "capability": "lint",
        "doc": "Additive Stylelint lint aspect.",
        "shard": "stylelint",
    },
    "real_js_typecheck": {
        "capability": "typecheck",
        "doc": "Additive TypeScript typecheck family aspect (delegated tsc over typescript_library and js_library).",
        "requires": [tsc_aspect],
        "shard": "js",
    },
}

def _tsc_hook(target, _ctx, resolved, _capability, _what, _shared):
    if TscInfo not in target:
        return without(resolved, "tsc")
    info = target[TscInfo]
    return real_context(
        upstream = info.logs,
        late_tool_files = [("dx-tsc/exit/%d" % i, status) for i, status in enumerate(info.statuses)],
        late_inputs = info.logs + info.statuses,
    )

def _eslint_hook(target, _ctx, _resolved, _capability, _what, _shared):
    if TscInfo not in target or target[TscInfo].tsconfig == None:
        return real_context()
    return real_context(
        late_staged = [target[TscInfo].inputs],
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
)

real_js_eslint_lint_aspect = real_make_aspect(_CFG, "real_js_eslint_lint")

real_js_gplint_lint_aspect = real_make_aspect(_CFG, "real_js_gplint_lint")

real_js_graphql_eslint_lint_aspect = real_make_aspect(_CFG, "real_js_graphql_eslint_lint")

real_js_graphql_eslint_typecheck_aspect = real_make_aspect(_CFG, "real_js_graphql_eslint_typecheck")

real_js_prettier_format_aspect = real_make_aspect(_CFG, "real_js_prettier_format")

real_js_redocly_lint_aspect = real_make_aspect(_CFG, "real_js_redocly_lint")

real_js_remark_format_aspect = real_make_aspect(_CFG, "real_js_remark_format")

real_js_stylelint_lint_aspect = real_make_aspect(_CFG, "real_js_stylelint_lint")

real_js_typecheck_aspect = real_make_aspect(_CFG, "real_js_typecheck")

JsShardProbeInfo = provider(
    doc = "Shard names the js shard probe found for one target and capability.",
    fields = {
        "capability": "lint, format or typecheck",
        "shards": "sorted shard names whose tool filter keeps a resolved stage",
    },
)

QUALITY_CFG = _CFG

real_js_format_probe_aspect = real_make_probe_aspect(_CFG, "format", "js", JsShardProbeInfo)

real_js_lint_probe_aspect = real_make_probe_aspect(_CFG, "lint", "js", JsShardProbeInfo)

real_js_typecheck_probe_aspect = real_make_probe_aspect(_CFG, "typecheck", "js", JsShardProbeInfo)
