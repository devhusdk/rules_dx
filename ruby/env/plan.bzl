"""Focused Ruby environment plan."""

load("//env:plan_factory.bzl", "simple_env_plan_rule")
load("//quality:sources.bzl", "QualitySourcesInfo")

RubyEnvPlanInfo = provider(
    doc = "Provider-derived focused Ruby target environment plan.",
    fields = {
        "direct_sources": "Sorted basenames of direct Ruby sources.",
        "has_sources": "Whether the wrapper owns any direct Ruby sources.",
        "source_count": "Number of direct Ruby sources.",
        "target": "Display label of the planned wrapper target.",
    },
)

ruby_env_plan = simple_env_plan_rule(
    rule_name = "ruby_env_plan",
    info = RubyEnvPlanInfo,
    required = [(QualitySourcesInfo, "QualitySourcesInfo")],
)
