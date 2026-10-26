"""Minimal C# wrappers."""

load("@rules_dotnet//dotnet:defs.bzl", _csharp_binary = "csharp_binary", _csharp_library = "csharp_library", _csharp_test = "csharp_test")
load("@rules_dotnet//dotnet/private:providers.bzl", "DotnetAssemblyCompileInfo", "DotnetAssemblyRuntimeInfo")
load("//libs/starlark:wrapper.bzl", "DX_EXEC_PROVIDES", "dx_executable_forward_rule", "dx_lcov_merger_attr", "dx_library_forward_rule", "dx_setdefaults", "dx_wrap", "dx_wrap_binary", "dx_wrap_test")
load("//quality:sources.bzl", "QualitySourcesInfo")

_DX_CSHARP_LIBRARY_PROVIDES = [
    DotnetAssemblyCompileInfo,
    DotnetAssemblyRuntimeInfo,
    DefaultInfo,
    InstrumentedFilesInfo,
    QualitySourcesInfo,
]

_DX_CSHARP_SOURCE_SPECS = [("csharp", "cs")]
_DX_CSHARP_SOURCE_EXTS = [".cs"]

_csharp_library_forward = dx_library_forward_rule(
    provides = _DX_CSHARP_LIBRARY_PROVIDES,
    required_providers = [(DotnetAssemblyCompileInfo, "DotnetAssemblyCompileInfo"), (DotnetAssemblyRuntimeInfo, "DotnetAssemblyRuntimeInfo")],
    quality_specs = _DX_CSHARP_SOURCE_SPECS,
    what = "csharp_*",
    allow_files = _DX_CSHARP_SOURCE_EXTS,
    upstream_providers = [[DotnetAssemblyCompileInfo, DotnetAssemblyRuntimeInfo]],
    runtime = "besteffort",
)

_csharp_binary_forward = dx_executable_forward_rule(
    kind = "executable",
    provides = DX_EXEC_PROVIDES,
    required_providers = [],
    quality_specs = _DX_CSHARP_SOURCE_SPECS,
    what = "csharp_*",
    allow_files = _DX_CSHARP_SOURCE_EXTS,
    upstream_providers = [[DotnetAssemblyCompileInfo, DotnetAssemblyRuntimeInfo]],
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
    extra_attrs = dx_lcov_merger_attr(),
    optional_providers = [DotnetAssemblyCompileInfo, DotnetAssemblyRuntimeInfo],
    runtime = "besteffort",
)

def csharp_tfm_with_defaults(kwargs):
    """Returns kwargs defaulting target_frameworks plus warnings-as-errors."""
    return dx_setdefaults(kwargs, {"target_frameworks": ["net10.0"], "treat_warnings_as_errors": True})

def csharp_library(name, srcs, visibility = None, **kwargs):
    """Minimal wrapper over csharp_library; target_frameworks defaults to net10.0 and warnings are errors."""
    dx_wrap(name, _csharp_library, _csharp_library_forward, srcs, visibility = visibility, upstream_kwargs = csharp_tfm_with_defaults(kwargs), **kwargs)

def csharp_binary(name, srcs = None, visibility = None, **kwargs):
    """Minimal wrapper over csharp_binary; srcs is optional, target_frameworks defaults to net10.0 and warnings are errors."""
    dx_wrap_binary(name, _csharp_binary, _csharp_binary_forward, srcs, visibility = visibility, upstream_kwargs = csharp_tfm_with_defaults(kwargs), **kwargs)

def csharp_test(name, srcs, visibility = None, **kwargs):
    """Minimal wrapper over csharp_test; target_frameworks defaults to net10.0 and warnings are errors."""
    dx_wrap_test(name, _csharp_test, _csharp_forward_test, srcs, visibility = visibility, upstream_kwargs = csharp_tfm_with_defaults(kwargs), **kwargs)
