"""Experimental minimal Zig wrappers."""

load("@rules_zig//zig:defs.bzl", _zig_binary = "zig_binary", _zig_library = "zig_library", _zig_test = "zig_test")
load("//libs/starlark:wrapper.bzl", "dx_executable_forward_rule", "dx_library_forward_rule", "dx_wrap", "dx_wrap_binary", "dx_wrap_test")
load("//quality:sources.bzl", "QualitySourcesInfo")

_ZIG_SRCS = [".zig", ".zon"]

_DX_ZIG_LIBRARY_PROVIDES = [
    QualitySourcesInfo,
]

_DX_ZIG_EXEC_PROVIDES = [
    QualitySourcesInfo,
]

_DX_ZIG_SOURCE_SPECS = [
    ("zig", ["zig", "zon"]),
]

_zig_library_forward = dx_library_forward_rule(
    provides = _DX_ZIG_LIBRARY_PROVIDES,
    required_providers = [],
    upstream_providers = [[DefaultInfo]],
    quality_specs = _DX_ZIG_SOURCE_SPECS,
    what = "zig_*",
    allow_files = _ZIG_SRCS,
    runtime = "besteffort",
)

_zig_binary_forward = dx_executable_forward_rule(
    kind = "executable",
    provides = _DX_ZIG_EXEC_PROVIDES,
    required_providers = [],
    upstream_providers = [[DefaultInfo]],
    quality_specs = _DX_ZIG_SOURCE_SPECS,
    what = "zig_*",
    allow_files = _ZIG_SRCS,
    runtime = "besteffort",
)

_zig_forward_test = dx_executable_forward_rule(
    kind = "test",
    provides = _DX_ZIG_EXEC_PROVIDES,
    required_providers = [],
    upstream_providers = [[DefaultInfo]],
    quality_specs = _DX_ZIG_SOURCE_SPECS,
    what = "zig_*",
    allow_files = _ZIG_SRCS,
    runtime = "besteffort",
)

def _zig_wrap_library(name, srcs, visibility = None, **kwargs):
    effective = dict(kwargs)
    if len(srcs) > 0 and "main" not in effective:
        effective["main"] = srcs[0]
    dx_wrap_binary(name, _zig_library, _zig_library_forward, srcs, visibility = visibility, upstream_kwargs = effective)

def _zig_wrap_binary(name, srcs, visibility = None, **kwargs):
    effective = dict(kwargs)
    if len(srcs) > 0 and "main" not in effective:
        effective["main"] = srcs[0]
    dx_wrap_binary(name, _zig_binary, _zig_binary_forward, srcs, visibility = visibility, upstream_kwargs = effective)

def zig_library(name, srcs = None, main = None, visibility = None, **kwargs):
    """Experimental minimal wrapper over zig_library."""
    effective = dict(kwargs)
    if main != None:
        effective["main"] = main
    _zig_wrap_library(name, srcs if srcs != None else [], visibility = visibility, **effective)

def zig_binary(name, srcs, visibility = None, **kwargs):
    """Experimental minimal wrapper over zig_binary."""
    _zig_wrap_binary(name, srcs, visibility = visibility, **kwargs)

def zig_test(name, srcs, main = None, visibility = None, **kwargs):
    """Experimental minimal wrapper over zig_test."""
    effective = dict(kwargs)
    if main != None:
        effective["main"] = main
    elif len(srcs) > 0:
        effective["main"] = srcs[0]
    dx_wrap_test(name, _zig_test, _zig_forward_test, srcs, visibility = visibility, **effective)
