"""Focused Svelte environment plan."""

load("@aspect_rules_js//js:providers.bzl", _JsInfo = "JsInfo")
load("//env:plan_factory.bzl", "js_env_plan_rule")

SvelteEnvPlanInfo = provider(
    doc = "Provider-derived focused Svelte target environment plan.",
    fields = {
        "direct_sources": "Sorted basenames of direct Svelte sources.",
        "has_npm": "Whether the transitive npm closure is non-empty.",
        "npm_source_count": "Number of files in the transitive JsInfo npm_sources closure.",
        "target": "Display label of the planned wrapper target.",
        "transitive_sources": "Sorted basenames of transitive JsInfo sources.",
    },
)

svelte_env_plan = js_env_plan_rule(
    rule_name = "svelte_env_plan",
    info = SvelteEnvPlanInfo,
    JsInfo = _JsInfo,
)
