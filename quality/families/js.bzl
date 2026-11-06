"""JavaScript and TypeScript real quality aspects."""

load("//quality:family_hooks.bzl", "default_files_hook", "without")
load("//quality:native_config.bzl", "native_config_scratch_rel")
load("//quality:real_core.bzl", "real_config", "real_context", "real_make_aspect", "real_make_probe_aspect")
load("//quality:tsc.bzl", "TscInfo", "tsc_aspect")
load("//quality:unused_roots.bzl", "unused_closure_sources", "unused_roots_aspect", "unused_roots_context")
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
        "requires": [tsc_aspect],
        "shard": "js",
        "suffix": "-js",
    },
    "real_js_unused": {
        "capability": "unused",
        "doc": "Additive JavaScript unused-code family aspect (knip over the target's whole closure).",
        "requires": [unused_roots_aspect],
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

def _prettier_hook(target, _ctx, _resolved, _capability, _what, _shared):
    if SqlDialectInfo not in target:
        return real_context()
    return real_context(options = {"dialect": target[SqlDialectInfo].dialect})

_JS_EXTENSIONS = ["cjs", "cts", "js", "mjs", "mts", "ts", "tsx"]

_JS_CLASSES = {
    "cjs": "javascript",
    "cts": "typescript",
    "js": "javascript",
    "mjs": "javascript",
    "mts": "typescript",
    "ts": "typescript",
    "tsx": "tsx",
}

def _knip_hook(target, ctx, _resolved, _capability, _what, _shared):
    roots = unused_roots_context(target, ctx, _JS_EXTENSIONS)
    upstream = getattr(ctx.rule.attr, "upstream", None)
    if upstream == None or DefaultInfo not in upstream:
        return roots
    manifests = {}
    for f in upstream[DefaultInfo].default_runfiles.files.to_list():
        rel = native_config_scratch_rel(f.short_path)
        if f.is_source and f.basename == "package.json" and rel != None:
            manifests[rel] = f
    return real_context(
        tool_files = roots.tool_files,
        inputs = roots.inputs,
        late_tool_files = sorted(manifests.items()),
        late_inputs = manifests.values(),
    )

def _harness_sources(target, capability, direct_sources):
    return unused_closure_sources(target, capability, direct_sources, _JS_CLASSES)

HOOK_ORDER = ["eslint", "tsc", "prettier", "redocly", "knip"]

HOOKS = {
    "eslint": _eslint_hook,
    "tsc": _tsc_hook,
    "prettier": _prettier_hook,
    "redocly": default_files_hook(["yaml", "yml", "json"]),
    "knip": _knip_hook,
}

_CFG = real_config(
    tools = TOOLS,
    shard_specs = SHARD_SPECS,
    hook_order = HOOK_ORDER,
    context_hooks = HOOKS,
    source_recognizers = [],
    harness_sources = _harness_sources,
    toolchain = None,
    check_policy = False,
)

real_js_format_aspect = real_make_aspect(_CFG, "real_js_format")

real_js_lint_aspect = real_make_aspect(_CFG, "real_js_lint")

real_js_unused_aspect = real_make_aspect(_CFG, "real_js_unused")

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
