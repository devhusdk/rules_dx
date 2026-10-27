"""Minimal C# wrappers."""

load("@rules_dotnet//dotnet:defs.bzl", _csharp_binary = "csharp_binary", _csharp_library = "csharp_library", _csharp_test = "csharp_test")
load("@rules_dotnet//dotnet/private:providers.bzl", "DotnetAssemblyCompileInfo", "DotnetAssemblyRuntimeInfo")
load("//libs/starlark:wrapper.bzl", "DX_EXEC_PROVIDES", "dx_executable_forward_rule", "dx_lcov_merger_attr", "dx_library_forward_rule", "dx_setdefaults", "dx_wrap", "dx_wrap_binary", "dx_wrap_test")
load("//quality:sources.bzl", "QualitySourcesInfo")
load("//tools/coverage/dotnet:coverage.bzl", "dx_dotnet_coverage_aspect", "dx_dotnet_coverage_attrs", "dx_dotnet_test_coverage")

# Set False to tag csharp tests dx-no-coverage-collector instead of running coverlet.
CSHARP_COVERAGE_COLLECTOR = True

_DX_CSHARP_LIBRARY_PROVIDES = [
    DotnetAssemblyCompileInfo,
    DotnetAssemblyRuntimeInfo,
    DefaultInfo,
    InstrumentedFilesInfo,
    QualitySourcesInfo,
]

_DX_CSHARP_SOURCE_SPECS = [("csharp", "cs")]
_DX_CSHARP_SOURCE_EXTS = [".cs"]

def _dx_tfm_split_impl(_settings, attr):
    return {tfm: {"@rules_dotnet//dotnet:target_framework": tfm} for tfm in attr.dx_target_frameworks}

_dx_tfm_split = transition(
    implementation = _dx_tfm_split_impl,
    inputs = [],
    outputs = ["@rules_dotnet//dotnet:target_framework"],
)

_DX_CSHARP_TFM_ATTRS = {
    "dx_target_frameworks": attr.string_list(
        doc = "Every target framework the upstream declares; Roslyn analyzes each one.",
    ),
    "dx_tfm_variants": attr.label(
        cfg = _dx_tfm_split,
        doc = "The upstream once per declared target framework; only the Roslyn aspect reads it.",
    ),
}

_csharp_library_forward = dx_library_forward_rule(
    provides = _DX_CSHARP_LIBRARY_PROVIDES,
    required_providers = [(DotnetAssemblyCompileInfo, "DotnetAssemblyCompileInfo"), (DotnetAssemblyRuntimeInfo, "DotnetAssemblyRuntimeInfo")],
    quality_specs = _DX_CSHARP_SOURCE_SPECS,
    what = "csharp_*",
    allow_files = _DX_CSHARP_SOURCE_EXTS,
    upstream_providers = [[DotnetAssemblyCompileInfo, DotnetAssemblyRuntimeInfo]],
    extra_attrs = _DX_CSHARP_TFM_ATTRS,
    runtime = "besteffort",
    instrument_srcs = True,
)

_csharp_binary_forward = dx_executable_forward_rule(
    kind = "executable",
    provides = DX_EXEC_PROVIDES,
    required_providers = [],
    quality_specs = _DX_CSHARP_SOURCE_SPECS,
    what = "csharp_*",
    allow_files = _DX_CSHARP_SOURCE_EXTS,
    upstream_providers = [[DotnetAssemblyCompileInfo, DotnetAssemblyRuntimeInfo]],
    extra_attrs = _DX_CSHARP_TFM_ATTRS,
    optional_providers = [DotnetAssemblyCompileInfo, DotnetAssemblyRuntimeInfo],
    runtime = "besteffort",
)

_csharp_forward_test = dx_executable_forward_rule(
    kind = "test",
    provides = DX_EXEC_PROVIDES,
    required_providers = [],
    quality_specs = _DX_CSHARP_SOURCE_SPECS,
    what = "csharp_*",
    allow_files = _DX_CSHARP_SOURCE_EXTS,
    upstream_providers = [[DotnetAssemblyCompileInfo, DotnetAssemblyRuntimeInfo]],
    extra_attrs = dx_lcov_merger_attr() | dx_dotnet_coverage_attrs() | _DX_CSHARP_TFM_ATTRS,
    optional_providers = [DotnetAssemblyCompileInfo, DotnetAssemblyRuntimeInfo],
    runtime = "besteffort",
    upstream_aspects = [dx_dotnet_coverage_aspect],
    coverage_providers = dx_dotnet_test_coverage,
)

def csharp_tfm_with_defaults(kwargs):
    """Returns kwargs defaulting target_frameworks plus warnings-as-errors."""
    return dx_setdefaults(kwargs, {"target_frameworks": ["net10.0"], "treat_warnings_as_errors": True})

def csharp_tfm_forward_kwargs(name, kwargs):
    """Returns the forwarder kwargs that fan Roslyn out over every declared target framework.

    rules_dotnet compiles only the highest compatible framework, so a list literal with
    more than one entry gets a split edge to the upstream, one branch per framework.
    A single framework or a select() value adds nothing.
    """
    frameworks = kwargs.get("target_frameworks", None)
    if type(frameworks) != "list" or len(frameworks) < 2:
        return {}
    return {
        "dx_target_frameworks": frameworks,
        "dx_tfm_variants": ":" + name + "_upstream",
    }

def csharp_library(name, srcs, visibility = None, **kwargs):
    """Minimal wrapper over csharp_library; target_frameworks defaults to net10.0 and warnings are errors."""
    upstream = csharp_tfm_with_defaults(kwargs)
    dx_wrap(name, _csharp_library, _csharp_library_forward, srcs, visibility = visibility, upstream_kwargs = upstream, extra_forward_kwargs = csharp_tfm_forward_kwargs(name, upstream), **kwargs)

def csharp_binary(name, srcs = None, visibility = None, **kwargs):
    """Minimal wrapper over csharp_binary; srcs is optional, target_frameworks defaults to net10.0 and warnings are errors."""
    upstream = csharp_tfm_with_defaults(kwargs)
    dx_wrap_binary(name, _csharp_binary, _csharp_binary_forward, srcs, visibility = visibility, upstream_kwargs = upstream, extra_forward_kwargs = csharp_tfm_forward_kwargs(name, upstream), **kwargs)

def csharp_test(name, srcs, visibility = None, **kwargs):
    """Minimal wrapper over csharp_test; target_frameworks defaults to net10.0 and warnings are errors."""
    upstream = csharp_tfm_with_defaults(kwargs)
    dx_wrap_test(name, _csharp_test, _csharp_forward_test, srcs, visibility = visibility, upstream_kwargs = upstream, extra_forward_kwargs = csharp_tfm_forward_kwargs(name, upstream), coverage_collector = CSHARP_COVERAGE_COLLECTOR, **kwargs)
