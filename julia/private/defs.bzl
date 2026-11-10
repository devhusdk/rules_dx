"""Thin Julia wrappers over rules_julia."""

load("@rules_julia//julia:defs.bzl", _julia_binary = "julia_binary", _julia_library = "julia_library", _julia_standalone_binary = "julia_standalone_binary", _julia_test = "julia_test")
load("@rules_julia//julia:julia_info.bzl", _JuliaInfo = "JuliaInfo")
load("//julia/private/coverage:coverage.bzl", "dx_julia_coverage_aspect", "dx_julia_coverage_attrs", "dx_julia_test_coverage")
load("//libs/starlark:cells.bzl", "unsupported_constraint")
load("//libs/starlark:entry.bzl", "dx_entry_quality_srcs", "dx_infer_main", "dx_main_candidates")
load("//libs/starlark:wrapper.bzl", "DX_EXEC_PROVIDES", "dx_executable_forward_rule", "dx_language_kwargs", "dx_lcov_merger_attr", "dx_library_forward_rule", "dx_wrap", "dx_wrap_binary", "dx_wrap_test")
load("//quality:sources.bzl", "QualitySourcesInfo", "QualityToolsInfo")

JuliaInfo = _JuliaInfo

JULIA_QUALITY_TOOLS = {"julia_format": Label("//julia/private/tools:julia_format")}

JULIA_SOURCE_SPECS = [("julia", "jl")]

JULIA_SOURCE_EXTS = [".jl"]

JULIA_COMPATIBLE_WITH = select({
    Label("//julia/private:rules_julia_platform"): [],
    "//conditions:default": [Label("@platforms//:incompatible"), unsupported_constraint("//julia/private")],
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
    instrument_srcs = True,
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
    extra_attrs = dx_lcov_merger_attr() | dx_julia_coverage_attrs(),
    runtime = "besteffort",
    quality_tools = JULIA_QUALITY_TOOLS,
    upstream_aspects = [dx_julia_coverage_aspect],
    coverage_providers = dx_julia_test_coverage,
)

def julia_library(name, srcs = None, visibility = None, **kwargs):
    """Thin wrapper over julia_library."""
    kwargs = dx_language_kwargs(kwargs, compatible_with = JULIA_COMPATIBLE_WITH)
    dx_wrap(name, _julia_library, _julia_library_forward, srcs or [], visibility = visibility, upstream_kwargs = dict(kwargs, package_name = name), **kwargs)

def julia_binary(name, *, srcs = None, main = None, visibility = None, **kwargs):
    """Thin wrapper over julia_binary. main is name.jl, else the single src."""
    main = dx_infer_main("julia_binary", name, main, srcs, dx_main_candidates(name, ".jl"))
    kwargs = dx_language_kwargs(kwargs, compatible_with = JULIA_COMPATIBLE_WITH)
    upstream = dict(kwargs, package_name = name, main = main)
    dx_wrap_binary(name, _julia_binary, _julia_binary_forward, srcs, visibility = visibility, upstream_kwargs = upstream, quality_srcs = dx_entry_quality_srcs(main, srcs), **kwargs)

def julia_test(name, *, srcs = None, main = None, visibility = None, **kwargs):
    """Thin wrapper over julia_test. main is name.jl, else the single src; under coverage it adds --code-coverage=user."""
    main = dx_infer_main("julia_test", name, main, srcs, dx_main_candidates(name, ".jl"))
    kwargs = dx_language_kwargs(kwargs, compatible_with = JULIA_COMPATIBLE_WITH)
    upstream = dict(kwargs, package_name = name)
    upstream["julia_flags"] = (kwargs.get("julia_flags") or []) + select({
        Label("//julia/private/coverage:coverage_on"): ["--code-coverage=user"],
        "//conditions:default": [],
    })
    upstream["main"] = main
    dx_wrap_test(name, _julia_test, _julia_forward_test, srcs, visibility = visibility, upstream_kwargs = upstream, quality_srcs = dx_entry_quality_srcs(main, srcs), **kwargs)

def julia_standalone_binary(name, **kwargs):
    """Standalone binary from a dx julia_library that defines julia_main()::Cint."""
    kwargs = dx_language_kwargs(kwargs, compatible_with = JULIA_COMPATIBLE_WITH)
    _julia_standalone_binary(name = name, **kwargs)
