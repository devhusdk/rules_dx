"""Thin Julia wrappers over rules_julia."""

load("@rules_julia//julia:defs.bzl", _julia_binary = "julia_binary", _julia_library = "julia_library", _julia_test = "julia_test")
load("@rules_julia//julia:julia_info.bzl", _JuliaInfo = "JuliaInfo")
load("//libs/starlark:wrapper.bzl", "DX_EXEC_PROVIDES", "dx_executable_forward_rule", "dx_lcov_merger_attr", "dx_library_forward_rule", "dx_wrap", "dx_wrap_binary", "dx_wrap_test")
load("//quality:sources.bzl", "QualitySourcesInfo", "QualityToolsInfo")

JuliaInfo = _JuliaInfo

JULIA_COVERAGE_COLLECTOR = False

JULIA_QUALITY_TOOLS = {"julia_format": Label("//julia/tools:julia_format")}

JULIA_SOURCE_SPECS = [("julia", "jl")]

JULIA_SOURCE_EXTS = [".jl"]

JULIA_COMPATIBLE_WITH = select({
    Label("//julia/rules:rules_julia_platform"): [],
    "//conditions:default": [Label("@platforms//:incompatible")],
})

_DX_JULIA_EXEC_PROVIDES = DX_EXEC_PROVIDES + [QualityToolsInfo, _JuliaInfo]

_julia_library_forward = dx_library_forward_rule(
    provides = [_JuliaInfo, DefaultInfo, InstrumentedFilesInfo, QualitySourcesInfo, QualityToolsInfo],
    required_providers = [(_JuliaInfo, "JuliaInfo")],
    quality_specs = JULIA_SOURCE_SPECS,
    what = "julia_*",
    allow_files = JULIA_SOURCE_EXTS,
    upstream_providers = [[_JuliaInfo]],
    runtime = "besteffort",
    quality_tools = JULIA_QUALITY_TOOLS,
)

_julia_binary_forward = dx_executable_forward_rule(
    kind = "executable",
    provides = _DX_JULIA_EXEC_PROVIDES,
    required_providers = [(_JuliaInfo, "JuliaInfo")],
    quality_specs = JULIA_SOURCE_SPECS,
    what = "julia_*",
    allow_files = JULIA_SOURCE_EXTS,
    upstream_providers = [[_JuliaInfo]],
    runtime = "besteffort",
    quality_tools = JULIA_QUALITY_TOOLS,
)

_julia_forward_test = dx_executable_forward_rule(
    kind = "test",
    provides = _DX_JULIA_EXEC_PROVIDES,
    required_providers = [(_JuliaInfo, "JuliaInfo")],
    quality_specs = JULIA_SOURCE_SPECS,
    what = "julia_*",
    allow_files = JULIA_SOURCE_EXTS,
    upstream_providers = [[_JuliaInfo]],
    extra_attrs = dx_lcov_merger_attr(),
    runtime = "besteffort",
    quality_tools = JULIA_QUALITY_TOOLS,
)

def julia_quality_srcs(main, srcs):
    """Returns the hand-written sources a Julia target owns: main first, then srcs."""
    owned = [main] if main != None else []
    return owned + [s for s in (srcs or []) if s != main]

def julia_compatible_with(target_compatible_with):
    """Appends the Julia platform guard to the caller's constraints."""
    return (target_compatible_with or []) + JULIA_COMPATIBLE_WITH

def _julia_kwargs(kwargs):
    return dict(kwargs, target_compatible_with = julia_compatible_with(kwargs.get("target_compatible_with")))

def julia_library(name, srcs = None, visibility = None, **kwargs):
    """Thin wrapper over julia_library."""
    dx_wrap(name, _julia_library, _julia_library_forward, srcs or [], visibility = visibility, **_julia_kwargs(kwargs))

def julia_binary(name, srcs = None, main = None, visibility = None, **kwargs):
    """Thin wrapper over julia_binary."""
    kwargs = _julia_kwargs(kwargs)
    upstream = dict(kwargs, main = main) if main != None else kwargs
    dx_wrap_binary(name, _julia_binary, _julia_binary_forward, srcs, visibility = visibility, upstream_kwargs = upstream, quality_srcs = julia_quality_srcs(main, srcs), **kwargs)

def julia_test(name, srcs = None, main = None, visibility = None, **kwargs):
    """Thin wrapper over julia_test."""
    kwargs = _julia_kwargs(kwargs)
    upstream = dict(kwargs, main = main) if main != None else kwargs
    dx_wrap_test(name, _julia_test, _julia_forward_test, srcs, visibility = visibility, upstream_kwargs = upstream, quality_srcs = julia_quality_srcs(main, srcs), coverage_collector = JULIA_COVERAGE_COLLECTOR, **kwargs)
