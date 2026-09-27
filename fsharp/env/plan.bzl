"""Focused F# environment plan."""

load("@rules_dotnet//dotnet/private:providers.bzl", "DotnetAssemblyCompileInfo", "DotnetAssemblyRuntimeInfo")
load("//env:focused.bzl", "focused_dotnet_transitive")
load("//env:plan_factory.bzl", "closure_env_plan_rule")
load("//quality:sources.bzl", "QualitySourcesInfo")

FSharpEnvPlanInfo = provider(
    doc = "Provider-derived focused F# target environment plan.",
    fields = {
        "direct_sources": "Sorted basenames of direct F# sources.",
        "has_sources": "Whether the wrapper owns any direct sources.",
        "has_tests": "Whether direct plus transitive closure carries test sources.",
        "source_count": "Number of direct sources.",
        "target": "Display label of the planned wrapper target.",
        "test_source_count": "Number of test sources in direct plus transitive closure.",
        "test_sources": "Sorted basenames of test sources in direct plus transitive closure.",
        "transitive_source_count": "Number of files in the transitive Dotnet assembly closure.",
        "transitive_sources": "Sorted basenames of the transitive Dotnet assembly closure.",
    },
)

def _fsharp_transitive(target):
    """Returns sorted basenames of the Dotnet assembly closure."""
    compile = target[DotnetAssemblyCompileInfo]
    return focused_dotnet_transitive(compile.refs, compile.transitive_refs)

fsharp_env_plan = closure_env_plan_rule(
    rule_name = "fsharp_env_plan",
    info = FSharpEnvPlanInfo,
    required = [(QualitySourcesInfo, "QualitySourcesInfo"), (DotnetAssemblyCompileInfo, "DotnetAssemblyCompileInfo"), (DotnetAssemblyRuntimeInfo, "DotnetAssemblyRuntimeInfo")],
    transitive = _fsharp_transitive,
)
