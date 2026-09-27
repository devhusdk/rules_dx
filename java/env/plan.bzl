"""Focused Java environment plan."""

load("@rules_java//java/common:java_info.bzl", "JavaInfo")
load("//env:focused.bzl", "focused_transitive_basenames")
load("//env:plan_factory.bzl", "closure_env_plan_rule")
load("//quality:sources.bzl", "QualitySourcesInfo")

JavaEnvPlanInfo = provider(
    doc = "Provider-derived focused Java target environment plan.",
    fields = {
        "direct_sources": "Sorted basenames of direct Java sources.",
        "has_sources": "Whether the wrapper owns any direct Java sources.",
        "has_tests": "Whether direct plus transitive closure carries test sources.",
        "source_count": "Number of direct Java sources.",
        "target": "Display label of the planned wrapper target.",
        "test_source_count": "Number of test sources in direct plus transitive closure.",
        "test_sources": "Sorted basenames of test sources in direct plus transitive closure.",
        "transitive_source_count": "Number of files in the transitive JavaInfo source-jar closure.",
        "transitive_sources": "Sorted basenames of the transitive JavaInfo source-jar closure.",
    },
)

def _java_transitive(target):
    """Returns sorted basenames of the JavaInfo source-jar closure."""
    return focused_transitive_basenames(target[JavaInfo].transitive_source_jars)

java_env_plan = closure_env_plan_rule(
    rule_name = "java_env_plan",
    info = JavaEnvPlanInfo,
    required = [(QualitySourcesInfo, "QualitySourcesInfo"), (JavaInfo, "JavaInfo")],
    transitive = _java_transitive,
)
