"""Focused Go environment plan."""

load("@rules_go//go:def.bzl", _GoArchive = "GoArchive")
load("//env:focused.bzl", "focused_go_transitive")
load("//env:plan_factory.bzl", "closure_env_plan_rule")
load("//quality:sources.bzl", "QualitySourcesInfo")

GoEnvPlanInfo = provider(
    doc = "Provider-derived focused Go target environment plan.",
    fields = {
        "direct_sources": "Sorted basenames of direct Go sources.",
        "has_sources": "Whether the wrapper owns any direct Go sources.",
        "has_tests": "Whether direct plus transitive closure carries test sources.",
        "source_count": "Number of direct Go sources.",
        "target": "Display label of the planned wrapper target.",
        "test_source_count": "Number of test sources in direct plus transitive closure.",
        "test_sources": "Sorted basenames of test sources in direct plus transitive closure.",
        "transitive_source_count": "Number of files in the transitive GoArchive source closure.",
        "transitive_sources": "Sorted basenames of the transitive GoArchive source closure.",
    },
)

def _go_transitive(target):
    """Returns sorted basenames of the GoArchive source closure."""
    return focused_go_transitive(target[_GoArchive].transitive)

go_env_plan = closure_env_plan_rule(
    rule_name = "go_env_plan",
    info = GoEnvPlanInfo,
    required = [(QualitySourcesInfo, "QualitySourcesInfo"), (_GoArchive, "GoArchive")],
    transitive = _go_transitive,
)
