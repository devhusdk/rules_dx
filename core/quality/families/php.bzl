"""PHP real quality aspects."""

load("//core/quality:family_hooks.bzl", "stage", "without")
load("//core/quality:real_core.bzl", "real_config", "real_context", "real_make_aspect", "real_make_probe_aspect")
load("//php/private:php.bzl", "external_path")
load("//php/private:providers.bzl", "PhpInfo")

visibility(["//core/...", "//tools/..."])

TOOLS = {
    "phpcs": {
        "attr": attr.label(default = "//config:tool_phpcs", allow_files = True, cfg = "exec"),
        "fallback": "=@rules_dx//php/private/tools:phpcs",
        "kind": "setting",
        "resources": "medium",
        "shard": "php",
    },
    "phpstan": {
        "attr": attr.label(default = "//config:tool_phpstan", allow_files = True, cfg = "exec"),
        "fallback": "=@rules_dx//php/private/tools:phpstan",
        "kind": "setting",
        "resources": "heavy",
        "shard": "php",
    },
}

_PHPSTAN_ATTRS = {
    "_phpstan_strict_rules": attr.label(default = "@dx_phpstan_strict_rules//:files", cfg = "exec", doc = "The pinned phpstan-strict-rules the strict profile includes."),
}

SHARD_SPECS = {
    "real_php_lint": {
        "capability": "lint",
        "doc": "Additive PHP lint family aspect (phpcs).",
        "shard": "php",
    },
    "real_php_typecheck": {
        "capability": "typecheck",
        "doc": "Additive PHP typecheck family aspect (phpstan).",
        "extra_attrs": _PHPSTAN_ATTRS,
        "shard": "php",
    },
}

def _phpstan_scan_line(f):
    return "dx-phpstan/scan/" + external_path(f.short_path) + "\t" + f.path

def _phpstan_pkg_line(f):
    if not f.path.endswith(".php"):
        return None
    return "dx-phpstan/pkg/" + external_path(f.short_path) + "\t" + f.path

def _phpstan_hook(target, ctx, resolved, _capability, _what, _shared):
    if PhpInfo not in target:
        return without(resolved, "phpstan")

    info = target[PhpInfo]
    tool_files = []
    inputs = []
    for f in ctx.files._phpstan_strict_rules:
        stage(tool_files, inputs, "vendor/phpstan/phpstan-strict-rules/" + f.short_path.split("/", 2)[2], f)
    return real_context(
        tool_files = tool_files,
        inputs = inputs,
        dep_lists = [
            (depset(transitive = [info.transitive_srcs, info.autoload_files]), _phpstan_scan_line),
            (info.package_files, _phpstan_pkg_line),
        ],
    )

HOOK_ORDER = ["phpstan"]

HOOKS = {"phpstan": _phpstan_hook}

_CFG = real_config(
    tools = TOOLS,
    shard_specs = SHARD_SPECS,
    hook_order = HOOK_ORDER,
    context_hooks = HOOKS,
)

real_php_lint_aspect = real_make_aspect(_CFG, "real_php_lint")

real_php_typecheck_aspect = real_make_aspect(_CFG, "real_php_typecheck")

PhpShardProbeInfo = provider(
    doc = "Shard names the php shard probe found for one target and capability.",
    fields = {
        "capability": "lint, format or typecheck",
        "shards": "sorted shard names whose tool filter keeps a resolved stage",
    },
)

QUALITY_CFG = _CFG

real_php_lint_probe_aspect = real_make_probe_aspect(_CFG, "lint", "php", PhpShardProbeInfo)

real_php_typecheck_probe_aspect = real_make_probe_aspect(_CFG, "typecheck", "php", PhpShardProbeInfo)
