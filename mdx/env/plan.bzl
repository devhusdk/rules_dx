"""Focused MDX environment plan."""

load("@aspect_rules_js//js:providers.bzl", _JsInfo = "JsInfo")
load("//env:plan_factory.bzl", "js_env_plan_rule")

MdxEnvPlanInfo = provider(
    doc = "Provider-derived focused MDX target environment plan.",
    fields = {
        "direct_sources": "Sorted basenames of direct MDX sources.",
        "has_npm": "Whether the transitive npm closure is non-empty.",
        "npm_source_count": "Number of files in the transitive JsInfo npm_sources closure.",
        "target": "Display label of the planned wrapper target.",
        "transitive_sources": "Sorted basenames of transitive JsInfo sources.",
    },
)

mdx_env_plan = js_env_plan_rule(
    rule_name = "mdx_env_plan",
    info = MdxEnvPlanInfo,
    JsInfo = _JsInfo,
)
