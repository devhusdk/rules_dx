"""Focused Astro environment plan."""

load("@aspect_rules_js//js:providers.bzl", _JsInfo = "JsInfo")
load("//env:plan_factory.bzl", "js_env_plan_rule")

AstroEnvPlanInfo = provider(
    doc = "Provider-derived focused Astro target environment plan.",
    fields = {
        "direct_sources": "Sorted basenames of direct Astro sources.",
        "has_npm": "Whether the transitive npm closure is non-empty.",
        "npm_source_count": "Number of files in the transitive JsInfo npm_sources closure.",
        "target": "Display label of the planned wrapper target.",
        "transitive_sources": "Sorted basenames of transitive JsInfo sources.",
    },
)

astro_env_plan = js_env_plan_rule(
    rule_name = "astro_env_plan",
    info = AstroEnvPlanInfo,
    JsInfo = _JsInfo,
)
