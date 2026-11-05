"""PHP real quality aspects."""

load("//php/rules:php.bzl", "staged_path")
load("//php/rules:providers.bzl", "PhpInfo")
load("//quality:family_hooks.bzl", "stage", "without")
load("//quality:real_core.bzl", "real_config", "real_context", "real_make_aspect")

TOOLS = {
    "phpcs": {
        "attr": attr.label(default = "//config:tool_phpcs", allow_files = True, cfg = "exec"),
        "fallback": "=@rules_dx//php/tools:phpcs",
        "kind": "setting",
        "resources": "medium",
        "shard": "php",
    },
    "phpstan": {
        "attr": attr.label(default = "//config:tool_phpstan", allow_files = True, cfg = "exec"),
        "fallback": "=@rules_dx//php/tools:phpstan",
        "kind": "setting",
        "resources": "heavy",
        "shard": "php",
    },
}

_PHPSTAN_ATTRS = {
    "_phpstan_unused_public": attr.label(default = "@dx_unused_public//:files", cfg = "exec", doc = "The pinned unused-public extension."),
    "_phpstan_assert": attr.label(default = "@dx_webmozart_assert//:files", cfg = "exec", doc = "The pinned webmozart/assert the extension needs."),
    "_phpstan_autoload": attr.label(default = "//php/tools:phpstan_autoload.php", allow_single_file = True, cfg = "exec", doc = "Autoloads the extension classes inside the phar run."),
}

SHARD_SPECS = {
    "real_php_lint": {
        "capability": "lint",
        "doc": "Additive PHP lint family aspect (phpcs).",
        "shard": "php",
        "suffix": "-php",
    },
    "real_php_typecheck": {
        "capability": "typecheck",
        "doc": "Additive PHP typecheck family aspect (phpstan).",
        "extra_attrs": _PHPSTAN_ATTRS,
        "shard": "php",
        "suffix": "-php",
    },
    "real_php_unused": {
        "capability": "unused",
        "doc": "Additive PHP unused-code family aspect (phpstan unused-public).",
        "extra_attrs": _PHPSTAN_ATTRS,
        "shard": "php",
        "suffix": "-php",
    },
}

def _phpstan_hook(target, ctx, resolved, capability, _what, shared):
    if PhpInfo not in target:
        return without(resolved, "phpstan")

    # Dependencies are scanned for their symbols, never analysed or run; the owned
    # sources are the CLI paths, so they stay out of the scan set.
    owned_paths = {f.path: True for f in shared.path_to_file.values()}
    info = target[PhpInfo]
    tool_files = []
    inputs = []
    seen_scan = {}
    for scan_file in sorted(info.transitive_srcs.to_list() + info.autoload_files.to_list(), key = lambda f: f.path):
        if scan_file.path not in owned_paths and scan_file.path not in seen_scan:
            seen_scan[scan_file.path] = True
            stage(tool_files, inputs, "dx-phpstan/scan/" + staged_path(scan_file), scan_file)
    for pkg_file in sorted(info.package_files.to_list(), key = lambda f: f.path):
        if pkg_file.path.endswith(".php") and pkg_file.path not in owned_paths and pkg_file.path not in seen_scan:
            seen_scan[pkg_file.path] = True
            stage(tool_files, inputs, "dx-phpstan/pkg/" + staged_path(pkg_file), pkg_file)
    if capability == "unused":
        for f in ctx.files._phpstan_unused_public:
            stage(tool_files, inputs, "dx-phpstan/ext/unused-public/" + f.short_path.split("/", 2)[2], f)
        for f in ctx.files._phpstan_assert:
            stage(tool_files, inputs, "dx-phpstan/ext/assert/" + f.short_path.split("/", 2)[2], f)
        stage(tool_files, inputs, "dx-phpstan/ext/autoload.php", ctx.file._phpstan_autoload)
    return real_context(tool_files = tool_files, inputs = inputs)

HOOK_ORDER = ["phpstan"]

HOOKS = {"phpstan": _phpstan_hook}

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

real_php_lint_aspect = real_make_aspect(_CFG, "real_php_lint")

real_php_typecheck_aspect = real_make_aspect(_CFG, "real_php_typecheck")

real_php_unused_aspect = real_make_aspect(_CFG, "real_php_unused")
