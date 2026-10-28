"""Minimal F# wrappers."""

load("@rules_dotnet//dotnet:defs.bzl", _fsharp_binary = "fsharp_binary", _fsharp_library = "fsharp_library", _fsharp_test = "fsharp_test")
load("@rules_dotnet//dotnet/private:providers.bzl", "DotnetAssemblyCompileInfo", "DotnetAssemblyRuntimeInfo")
load("//libs/starlark:wrapper.bzl", "DX_EXEC_PROVIDES", "dx_executable_forward_rule", "dx_lcov_merger_attr", "dx_library_forward_rule", "dx_setdefaults", "dx_wrap", "dx_wrap_binary", "dx_wrap_test")
load("//quality:sources.bzl", "QualitySourcesInfo")
load("//tools/coverage/dotnet:coverage.bzl", "dx_dotnet_coverage_aspect", "dx_dotnet_coverage_attrs", "dx_dotnet_test_coverage")

# Set False to tag fsharp tests dx-no-coverage-collector instead of running coverlet.
FSHARP_COVERAGE_COLLECTOR = True

_DX_FSHARP_LIBRARY_PROVIDES = [
    DotnetAssemblyCompileInfo,
    DotnetAssemblyRuntimeInfo,
    DefaultInfo,
    InstrumentedFilesInfo,
    QualitySourcesInfo,
]

_DX_FSHARP_SOURCE_SPECS = [("fsharp", ["fs", "fsi"])]
_DX_FSHARP_SOURCE_EXTS = [".fs", ".fsi"]

_DX_FSHARP_CONTEXT_ATTRS = {
    "defines": attr.string_list(doc = "User compile symbols of the target."),
    "langversion": attr.string(doc = "F# language version of the target."),
    "target_frameworks": attr.string_list(doc = "Target frameworks of the target."),
    "_target_framework": attr.label(default = "@rules_dotnet//dotnet:target_framework", doc = "Incoming target framework setting."),
}

_fsharp_library_forward = dx_library_forward_rule(
    provides = _DX_FSHARP_LIBRARY_PROVIDES,
    extra_attrs = _DX_FSHARP_CONTEXT_ATTRS,
    required_providers = [(DotnetAssemblyCompileInfo, "DotnetAssemblyCompileInfo"), (DotnetAssemblyRuntimeInfo, "DotnetAssemblyRuntimeInfo")],
    quality_specs = _DX_FSHARP_SOURCE_SPECS,
    what = "fsharp_*",
    allow_files = _DX_FSHARP_SOURCE_EXTS,
    upstream_providers = [[DotnetAssemblyCompileInfo, DotnetAssemblyRuntimeInfo]],
    runtime = "besteffort",
    instrument_srcs = True,
)

_fsharp_binary_forward = dx_executable_forward_rule(
    kind = "executable",
    extra_attrs = _DX_FSHARP_CONTEXT_ATTRS,
    provides = DX_EXEC_PROVIDES,
    required_providers = [],
    quality_specs = _DX_FSHARP_SOURCE_SPECS,
    what = "fsharp_*",
    allow_files = _DX_FSHARP_SOURCE_EXTS,
    upstream_providers = [[DotnetAssemblyCompileInfo, DotnetAssemblyRuntimeInfo]],
    optional_providers = [DotnetAssemblyCompileInfo, DotnetAssemblyRuntimeInfo],
    runtime = "besteffort",
)

_fsharp_forward_test = dx_executable_forward_rule(
    kind = "test",
    provides = DX_EXEC_PROVIDES,
    required_providers = [],
    quality_specs = _DX_FSHARP_SOURCE_SPECS,
    what = "fsharp_*",
    allow_files = _DX_FSHARP_SOURCE_EXTS,
    upstream_providers = [[DotnetAssemblyCompileInfo, DotnetAssemblyRuntimeInfo]],
    extra_attrs = _DX_FSHARP_CONTEXT_ATTRS | dx_lcov_merger_attr() | dx_dotnet_coverage_attrs(),
    optional_providers = [DotnetAssemblyCompileInfo, DotnetAssemblyRuntimeInfo],
    runtime = "besteffort",
    upstream_aspects = [dx_dotnet_coverage_aspect],
    coverage_providers = dx_dotnet_test_coverage,
)

def fsharp_tfm_with_defaults(kwargs):
    """Returns kwargs defaulting target_frameworks plus warnings-as-errors."""
    return dx_setdefaults(kwargs, {"target_frameworks": ["net10.0"], "treat_warnings_as_errors": True})

def _fsharp_context(kwargs):
    return {
        "defines": kwargs.get("defines"),
        "langversion": kwargs.get("langversion"),
        "target_frameworks": fsharp_tfm_with_defaults(kwargs)["target_frameworks"],
    }

def fsharp_library(name, srcs, visibility = None, **kwargs):
    """Minimal wrapper over fsharp_library; srcs compile in list order, target_frameworks defaults to net10.0 and warnings are errors."""
    dx_wrap(name, _fsharp_library, _fsharp_library_forward, srcs, visibility = visibility, upstream_kwargs = fsharp_tfm_with_defaults(kwargs), extra_forward_kwargs = _fsharp_context(kwargs), **kwargs)

def fsharp_binary(name, srcs = None, visibility = None, upstream_visibility = None, **kwargs):
    """Minimal wrapper over fsharp_binary; srcs is optional and compiles in list order, target_frameworks defaults to net10.0 and warnings are errors."""
    dx_wrap_binary(name, _fsharp_binary, _fsharp_binary_forward, srcs, visibility = visibility, upstream_visibility = upstream_visibility, upstream_kwargs = fsharp_tfm_with_defaults(kwargs), extra_forward_kwargs = _fsharp_context(kwargs), **kwargs)

def fsharp_test(name, srcs, visibility = None, **kwargs):
    """Minimal wrapper over fsharp_test; srcs compile in list order, target_frameworks defaults to net10.0 and warnings are errors."""
    dx_wrap_test(name, _fsharp_test, _fsharp_forward_test, srcs, visibility = visibility, upstream_kwargs = fsharp_tfm_with_defaults(kwargs), extra_forward_kwargs = _fsharp_context(kwargs), coverage_collector = FSHARP_COVERAGE_COLLECTOR, **kwargs)
