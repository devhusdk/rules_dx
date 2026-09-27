"""Focused TypeScript environment plan."""

load("@aspect_rules_js//js:providers.bzl", _JsInfo = "JsInfo")
load("@aspect_rules_ts//ts:defs.bzl", _TsConfigInfo = "TsConfigInfo")
load("//env:plan_factory.bzl", "js_env_plan_rule")

TypeScriptEnvPlanInfo = provider(
    doc = "Provider-derived focused TypeScript target environment plan.",
    fields = {
        "direct_sources": "Sorted basenames of direct TypeScript sources.",
        "has_npm": "Whether the transitive npm closure is non-empty.",
        "has_store": "Whether the pnpm store closure is non-empty.",
        "has_tsconfig": "Whether the target preserves TsConfigInfo.",
        "npm_source_count": "Number of files in the transitive JsInfo npm_sources closure.",
        "store_count": "Number of entries in the JsInfo npm_package_store_infos closure.",
        "target": "Display label of the planned wrapper target.",
        "transitive_sources": "Sorted basenames of transitive JsInfo sources.",
        "tsconfig": "Sorted basenames of TsConfigInfo deps, else empty.",
        "tsconfig_count": "Number of files in the TsConfigInfo deps closure.",
    },
)

typescript_env_plan = js_env_plan_rule(
    rule_name = "typescript_env_plan",
    info = TypeScriptEnvPlanInfo,
    JsInfo = _JsInfo,
    TsConfigInfo = _TsConfigInfo,
    with_store = True,
    with_tsconfig = True,
)
