"""Minimal C# wrappers."""

load("@rules_dotnet//dotnet:defs.bzl", _csharp_binary = "csharp_binary", _csharp_library = "csharp_library", _csharp_test = "csharp_test")
load("@rules_dotnet//dotnet/private:providers.bzl", "DotnetAssemblyCompileInfo", "DotnetAssemblyRuntimeInfo")
load("//libs/starlark:wrapper.bzl", "dx_executable_forward_rule", "dx_lcov_merger_attr", "dx_library_forward_rule", "dx_wrap", "dx_wrap_binary", "dx_wrap_test")
load("//quality:sources.bzl", "QualitySourcesInfo")

_DX_CSHARP_LIBRARY_PROVIDES = [
    DotnetAssemblyCompileInfo,
    DotnetAssemblyRuntimeInfo,
    DefaultInfo,
    InstrumentedFilesInfo,
    QualitySourcesInfo,
]

_DX_CSHARP_EXEC_PROVIDES = [
    DefaultInfo,
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
    provides = _DX_CSHARP_EXEC_PROVIDES,
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
    provides = _DX_CSHARP_EXEC_PROVIDES,
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
    upstream_kwargs = dict(kwargs)
    upstream_kwargs.setdefault("target_frameworks", ["net10.0"])
    upstream_kwargs.setdefault("treat_warnings_as_errors", True)
    return upstream_kwargs

def _csharp_with_tfm(kwargs):
    return csharp_tfm_with_defaults(kwargs)

def _csharp_wrap_library(name, srcs, visibility = None, **kwargs):
    dx_wrap(name, _csharp_library, _csharp_library_forward, srcs, visibility = visibility, **_csharp_with_tfm(kwargs))

def _csharp_wrap_binary(name, srcs, visibility = None, **kwargs):
    dx_wrap_binary(name, _csharp_binary, _csharp_binary_forward, srcs, visibility = visibility, upstream_kwargs = _csharp_with_tfm(kwargs), **kwargs)

def csharp_library(name, srcs, visibility = None, **kwargs):
    """Minimal wrapper over csharp_library."""
    _csharp_wrap_library(name, srcs, visibility = visibility, **kwargs)

def csharp_binary(name, srcs = None, visibility = None, **kwargs):
    """Minimal wrapper over csharp_binary."""
    effective_srcs = srcs if srcs != None else []
    _csharp_wrap_binary(name, effective_srcs, visibility = visibility, **kwargs)

def csharp_test(name, srcs, visibility = None, **kwargs):
    """Minimal wrapper over csharp_test."""
    dx_wrap_test(name, _csharp_test, _csharp_forward_test, srcs, visibility = visibility, upstream_kwargs = _csharp_with_tfm(kwargs), **kwargs)
