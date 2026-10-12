"""Minimal F# wrappers."""

load("@rules_dotnet//dotnet:defs.bzl", _fsharp_binary = "fsharp_binary", _fsharp_library = "fsharp_library", _fsharp_test = "fsharp_test")
load("@rules_dotnet//dotnet/private:providers.bzl", "DotnetAssemblyCompileInfo", "DotnetAssemblyRuntimeInfo")
load("//libs/starlark:wrapper.bzl", "dx_executable_forward_rule", "dx_lcov_merger_attr", "dx_library_forward_rule", "dx_wrap", "dx_wrap_binary", "dx_wrap_test")
load("//quality:sources.bzl", "QualitySourcesInfo")

_DX_FSHARP_LIBRARY_PROVIDES = [
    DotnetAssemblyCompileInfo,
    DotnetAssemblyRuntimeInfo,
    DefaultInfo,
    InstrumentedFilesInfo,
    QualitySourcesInfo,
]

_DX_FSHARP_EXEC_PROVIDES = [
    DefaultInfo,
    QualitySourcesInfo,
]

_DX_FSHARP_SOURCE_SPECS = [("fsharp", ["fs", "fsi"])]
_DX_FSHARP_SOURCE_EXTS = [".fs", ".fsi"]

_fsharp_library_forward = dx_library_forward_rule(
    provides = _DX_FSHARP_LIBRARY_PROVIDES,
    required_providers = [(DotnetAssemblyCompileInfo, "DotnetAssemblyCompileInfo"), (DotnetAssemblyRuntimeInfo, "DotnetAssemblyRuntimeInfo")],
    quality_specs = _DX_FSHARP_SOURCE_SPECS,
    what = "fsharp_*",
    allow_files = _DX_FSHARP_SOURCE_EXTS,
    upstream_providers = [[DotnetAssemblyCompileInfo, DotnetAssemblyRuntimeInfo]],
    runtime = "besteffort",
)

_fsharp_binary_forward = dx_executable_forward_rule(
    kind = "executable",
    provides = _DX_FSHARP_EXEC_PROVIDES,
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
    provides = _DX_FSHARP_EXEC_PROVIDES,
    required_providers = [],
    quality_specs = _DX_FSHARP_SOURCE_SPECS,
    what = "fsharp_*",
    allow_files = _DX_FSHARP_SOURCE_EXTS,
    upstream_providers = [[DotnetAssemblyCompileInfo, DotnetAssemblyRuntimeInfo]],
    extra_attrs = dx_lcov_merger_attr(),
    optional_providers = [DotnetAssemblyCompileInfo, DotnetAssemblyRuntimeInfo],
    runtime = "besteffort",
)

def fsharp_tfm_with_defaults(kwargs):
    """Returns kwargs defaulting target_frameworks plus warnings-as-errors."""
    upstream_kwargs = dict(kwargs)
    upstream_kwargs.setdefault("target_frameworks", ["net10.0"])
    upstream_kwargs.setdefault("treat_warnings_as_errors", True)
    return upstream_kwargs

def _fsharp_with_tfm(kwargs):
    return fsharp_tfm_with_defaults(kwargs)

def _fsharp_wrap_library(name, srcs, visibility = None, **kwargs):
    dx_wrap(name, _fsharp_library, _fsharp_library_forward, srcs, visibility = visibility, **_fsharp_with_tfm(kwargs))

def _fsharp_wrap_binary(name, srcs, visibility = None, **kwargs):
    dx_wrap_binary(name, _fsharp_binary, _fsharp_binary_forward, srcs, visibility = visibility, upstream_kwargs = _fsharp_with_tfm(kwargs), **kwargs)

def fsharp_library(name, srcs, visibility = None, **kwargs):
    """Minimal wrapper over fsharp_library."""
    _fsharp_wrap_library(name, srcs, visibility = visibility, **kwargs)

def fsharp_binary(name, srcs = None, visibility = None, **kwargs):
    """Minimal wrapper over fsharp_binary."""
    effective_srcs = srcs if srcs != None else []
    _fsharp_wrap_binary(name, effective_srcs, visibility = visibility, **kwargs)

def fsharp_test(name, srcs, visibility = None, **kwargs):
    """Minimal wrapper over fsharp_test."""
    dx_wrap_test(name, _fsharp_test, _fsharp_forward_test, srcs, visibility = visibility, upstream_kwargs = _fsharp_with_tfm(kwargs), **kwargs)
