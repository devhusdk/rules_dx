"""Kotlin real quality aspects."""

load("//core/quality:real_core.bzl", "real_config", "real_make_aspect", "real_make_probe_aspect")
load("//java/private/quality:jvm.bzl", "RECOGNIZER", "jvm_check_hook")
load(":kotlinc_check.bzl", "kotlinc_check_aspect")

TOOLS = {
    "detekt": {
        "attr": attr.label(default = "//java/private/quality/tools:detekt", cfg = "exec", executable = True),
        "capabilities": ["lint"],
        "kind": "executable",
        "resources": "heavy",
        "shard": "kotlin",
    },
    "kotlinc": {
        "capabilities": ["typecheck"],
        "kind": "upstream",
        "resources": "heavy",
        "shard": "kotlin",
    },
    "ktfmt": {
        "attr": attr.label(default = "//java/private/quality/tools:ktfmt", cfg = "exec", executable = True),
        "capabilities": ["format"],
        "kind": "executable",
        "resources": "heavy",
        "shard": "kotlin",
    },
}

SHARD_SPECS = {
    "real_kotlin_format": {
        "capability": "format",
        "doc": "Additive Kotlin format family aspect (ktfmt).",
        "shard": "kotlin",
    },
    "real_kotlin_lint": {
        "capability": "lint",
        "doc": "Additive Kotlin lint family aspect (detekt).",
        "shard": "kotlin",
    },
    "real_kotlin_typecheck": {
        "capability": "typecheck",
        "doc": "Additive Kotlin typecheck family aspect (delegated kotlinc over rules_kotlin targets).",
        "requires": [kotlinc_check_aspect],
        "shard": "kotlin",
    },
}

HOOK_ORDER = ["kotlinc"]

HOOKS = {
    "kotlinc": jvm_check_hook("kotlinc"),
}

_CFG = real_config(
    tools = TOOLS,
    shard_specs = SHARD_SPECS,
    hook_order = HOOK_ORDER,
    context_hooks = HOOKS,
    source_recognizers = [RECOGNIZER],
)

real_kotlin_format_aspect = real_make_aspect(_CFG, "real_kotlin_format")

real_kotlin_lint_aspect = real_make_aspect(_CFG, "real_kotlin_lint")

real_kotlin_typecheck_aspect = real_make_aspect(_CFG, "real_kotlin_typecheck")

KotlinShardProbeInfo = provider(
    doc = "Shard names the kotlin shard probe found for one target and capability.",
    fields = {
        "capability": "lint, format or typecheck",
        "shards": "sorted shard names whose tool filter keeps a resolved stage",
    },
)

QUALITY_CFG = _CFG

real_kotlin_format_probe_aspect = real_make_probe_aspect(_CFG, "format", "kotlin", KotlinShardProbeInfo)

real_kotlin_lint_probe_aspect = real_make_probe_aspect(_CFG, "lint", "kotlin", KotlinShardProbeInfo)

real_kotlin_typecheck_probe_aspect = real_make_probe_aspect(_CFG, "typecheck", "kotlin", KotlinShardProbeInfo)
