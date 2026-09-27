"""Focused Vue environment plan."""

load("@aspect_rules_js//js:providers.bzl", _JsInfo = "JsInfo")
load("//env:plan_factory.bzl", "js_env_plan_rule")

VueEnvPlanInfo = provider(
    doc = "Provider-derived focused Vue target environment plan.",
    fields = {
        "direct_sources": "Sorted basenames of direct Vue sources.",
        "has_npm": "Whether the transitive npm closure is non-empty.",
        "npm_source_count": "Number of files in the transitive JsInfo npm_sources closure.",
        "target": "Display label of the planned wrapper target.",
        "transitive_sources": "Sorted basenames of transitive JsInfo sources.",
    },
)

vue_env_plan = js_env_plan_rule(
    rule_name = "vue_env_plan",
    info = VueEnvPlanInfo,
    JsInfo = _JsInfo,
)
