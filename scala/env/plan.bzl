"""Focused Scala environment plan."""

load("@rules_java//java:defs.bzl", "JavaInfo")
load("//env:focused.bzl", "focused_transitive_basenames")
load("//env:plan_factory.bzl", "closure_env_plan_rule")
load("//quality:sources.bzl", "QualitySourcesInfo")

ScalaEnvPlanInfo = provider(
    doc = "Provider-derived focused Scala target environment plan.",
    fields = {
        "direct_sources": "Sorted basenames of direct Scala/Java sources.",
        "has_sources": "Whether the wrapper owns any direct sources.",
        "has_tests": "Whether direct plus transitive closure carries test sources.",
        "source_count": "Number of direct sources.",
        "target": "Display label of the planned wrapper target.",
        "test_source_count": "Number of test sources in direct plus transitive closure.",
        "test_sources": "Sorted basenames of test sources in direct plus transitive closure.",
        "transitive_source_count": "Number of files in the transitive JavaInfo source-jar closure.",
        "transitive_sources": "Sorted basenames of the transitive JavaInfo source-jar closure.",
    },
)

def _scala_transitive(target):
    """Returns sorted basenames of the JavaInfo source-jar closure."""
    return focused_transitive_basenames(target[JavaInfo].transitive_source_jars)

scala_env_plan = closure_env_plan_rule(
    rule_name = "scala_env_plan",
    info = ScalaEnvPlanInfo,
    required = [(QualitySourcesInfo, "QualitySourcesInfo"), (JavaInfo, "JavaInfo")],
    transitive = _scala_transitive,
)
