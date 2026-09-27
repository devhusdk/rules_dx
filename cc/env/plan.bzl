"""Focused C/C++ environment plan."""

load("@rules_cc//cc/common:cc_info.bzl", "CcInfo")
load("//env:focused.bzl", "focused_transitive_basenames")
load("//env:plan_factory.bzl", "closure_env_plan_rule")
load("//quality:sources.bzl", "QualitySourcesInfo")

CcEnvPlanInfo = provider(
    doc = "Provider-derived focused C/C++ target environment plan.",
    fields = {
        "direct_sources": "Sorted basenames of direct C/C++ sources.",
        "has_sources": "Whether the wrapper owns any direct C/C++ sources.",
        "has_tests": "Whether direct plus transitive closure carries test sources.",
        "source_count": "Number of direct C/C++ sources.",
        "target": "Display label of the planned wrapper target.",
        "test_source_count": "Number of test sources in direct plus transitive closure.",
        "test_sources": "Sorted basenames of test sources in direct plus transitive closure.",
        "transitive_source_count": "Number of files in the transitive CcInfo header closure.",
        "transitive_sources": "Sorted basenames of the transitive CcInfo header closure.",
    },
)

def _cc_transitive(target):
    """Returns sorted basenames of the CcInfo header closure."""
    return focused_transitive_basenames(target[CcInfo].compilation_context.headers)

cc_env_plan = closure_env_plan_rule(
    rule_name = "cc_env_plan",
    info = CcEnvPlanInfo,
    required = [(QualitySourcesInfo, "QualitySourcesInfo"), (CcInfo, "CcInfo")],
    transitive = _cc_transitive,
)
