"""JavaScript and TypeScript real quality aspects."""

load("//quality:family_hooks.bzl", "default_files_hook", "without")
load("//quality:real_core.bzl", "real_config", "real_context", "real_make_aspect")
load("//quality:tsc.bzl", "TscInfo", "tsc_aspect")
load("//sql/rules:providers.bzl", "SqlDialectInfo")

TOOLS = {
    "eslint": {
        "attr": attr.label(default = "//quality/tools/javascript/bin:eslint", cfg = "exec", executable = True),
        "env": [["JS_BINARY__NO_CD_BINDIR", "1"]],
        "extra": "files",
        "kind": "executable",
        "resources": "heavy",
        "shard": "js",
    },
    "gplint": {
        "attr": attr.label(default = "//quality/tools/javascript/bin:gplint", cfg = "exec", executable = True),
        "env": [["JS_BINARY__NO_CD_BINDIR", "1"]],
        "extra": "files",
        "kind": "executable",
        "resources": "heavy",
        "shard": "js",
    },
    "graphql_eslint": {
        "attr": attr.label(default = "//quality/tools/javascript/bin:graphql_eslint", cfg = "exec", executable = True),
        "env": [["JS_BINARY__NO_CD_BINDIR", "1"]],
        "extra": "files",
        "kind": "executable",
        "resources": "heavy",
        "shard": "js",
    },
    "knip": {
        "attr": attr.label(default = "//quality/tools/javascript/bin:knip", cfg = "exec", executable = True),
        "capabilities": ["unused"],
        "env": [["JS_BINARY__NO_CD_BINDIR", "1"]],
        "kind": "executable",
        "resources": "heavy",
        "shard": "js",
    },
    "prettier": {
        "attr": attr.label(default = "//quality/tools/javascript/bin:prettier", cfg = "exec", executable = True),
        "env": [["JS_BINARY__NO_CD_BINDIR", "1"]],
        "extra": "files",
        "kind": "executable",
        "resources": "heavy",
        "shard": "js",
    },
    "redocly": {
        "attr": attr.label(default = "//quality/tools/javascript/bin:redocly", cfg = "exec", executable = True),
        "env": [["JS_BINARY__NO_CD_BINDIR", "1"]],
        "extra": "files",
        "kind": "executable",
        "resources": "medium",
        "shard": "js",
    },
    "remark": {
        "attr": attr.label(default = "//quality/tools/javascript/bin:remark", cfg = "exec", executable = True),
        "env": [["JS_BINARY__NO_CD_BINDIR", "1"]],
        "extra": "files",
        "kind": "executable",
        "resources": "heavy",
        "shard": "js",
    },
    "stylelint": {
        "attr": attr.label(default = "//quality/tools/javascript/bin:stylelint", cfg = "exec", executable = True),
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
        "shard": "js",
        "suffix": "-js",
    },
    "real_js_unused": {
        "capability": "unused",
        "doc": "Additive JavaScript unused-code family aspect (knip).",
        "shard": "js",
        "suffix": "-js",
    },
    "real_js_typecheck": {
        "capability": "typecheck",
        "doc": "Additive TypeScript typecheck family aspect (delegated tsc over typescript_project and js_library).",
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

def _prettier_hook(target, _ctx, _resolved, _capability, _what, _shared):
    if SqlDialectInfo not in target:
        return real_context()
    return real_context(options = {"dialect": target[SqlDialectInfo].dialect})

HOOK_ORDER = ["tsc", "prettier", "redocly"]

HOOKS = {
    "tsc": _tsc_hook,
    "prettier": _prettier_hook,
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

real_js_unused_aspect = real_make_aspect(_CFG, "real_js_unused")

real_js_typecheck_aspect = real_make_aspect(_CFG, "real_js_typecheck")
