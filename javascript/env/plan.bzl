"""Focused JavaScript environment plan."""

load("@aspect_rules_js//js:providers.bzl", _JsInfo = "JsInfo")
load("//env:plan_factory.bzl", "js_env_plan_rule")

JavaScriptEnvPlanInfo = provider(
    doc = "Provider-derived focused JavaScript target environment plan.",
    fields = {
        "direct_sources": "Sorted basenames of direct JavaScript sources.",
        "has_npm": "Whether the transitive npm closure is non-empty.",
        "has_store": "Whether the pnpm store closure is non-empty.",
        "npm_source_count": "Number of files in the transitive JsInfo npm_sources closure.",
        "store_count": "Number of entries in the JsInfo npm_package_store_infos closure.",
        "target": "Display label of the planned wrapper target.",
        "transitive_sources": "Sorted basenames of transitive JsInfo sources.",
    },
)

javascript_env_plan = js_env_plan_rule(
    rule_name = "javascript_env_plan",
    info = JavaScriptEnvPlanInfo,
    JsInfo = _JsInfo,
    with_store = True,
)
