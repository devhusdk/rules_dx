"""Thin D wrappers over rules_d."""

load("@rules_d//d:defs.bzl", _d_binary = "d_binary", _d_library = "d_library", _d_test = "d_test")
load("@rules_d//d/private:providers.bzl", _DInfo = "DInfo")
load("//erlang/toolchain:pins.bzl", "CELLS")
load("//libs/starlark:wrapper.bzl", "DX_EXEC_PROVIDES", "dx_executable_forward_rule", "dx_lcov_merger_attr", "dx_library_forward_rule", "dx_upstream_tags", "dx_wrap", "dx_wrap_binary", "dx_wrap_test")
load("//quality:sources.bzl", "QualitySourcesInfo", "QualityToolsInfo")

DInfo = _DInfo

D_COMPATIBLE_WITH = [Label(constraint) for constraint in CELLS["linux_amd64"].exec_compatible_with]

D_QUALITY_TOOLS = {
    "dfmt": Label("//d/tools:dfmt"),
    "dscanner": Label("//d/tools:dscanner"),
}

_D_EXEC_PROVIDES = DX_EXEC_PROVIDES + [QualityToolsInfo]

_SPECS = [("d", "d")]

_EXTS = [".d", ".di"]

_d_library_forward = dx_library_forward_rule(
    provides = [_DInfo, DefaultInfo, InstrumentedFilesInfo, QualitySourcesInfo, QualityToolsInfo],
    required_providers = [(_DInfo, "DInfo")],
    quality_specs = _SPECS,
    what = "d_library",
    allow_files = _EXTS,
    upstream_providers = [[_DInfo]],
    runtime = "besteffort",
    quality_tools = D_QUALITY_TOOLS,
)

_d_binary_forward = dx_executable_forward_rule(
    kind = "executable",
    provides = _D_EXEC_PROVIDES,
    required_providers = [],
    quality_specs = _SPECS,
    what = "d_binary",
    allow_files = _EXTS,
    upstream_providers = None,
    runtime = "besteffort",
    quality_tools = D_QUALITY_TOOLS,
)

_d_forward_test = dx_executable_forward_rule(
    kind = "test",
    provides = _D_EXEC_PROVIDES,
    required_providers = [],
    quality_specs = _SPECS,
    what = "d_test",
    allow_files = _EXTS,
    upstream_providers = None,
    extra_attrs = dx_lcov_merger_attr(),
    runtime = "besteffort",
    quality_tools = D_QUALITY_TOOLS,
)

def d_quality_srcs(srcs, string_srcs = None):
    """Returns the hand-written D sources a target owns: srcs without any string import file."""
    excluded = string_srcs or []
    return [s for s in (srcs or []) if s not in excluded]

def d_library(name, srcs, visibility = None, **kwargs):
    """Thin wrapper over the rules_d d_library rule."""
    dx_wrap(name, _d_library, _d_library_forward, srcs, visibility = visibility, quality_srcs = d_quality_srcs(srcs, kwargs.get("string_srcs")), **kwargs)

def d_binary(name, srcs, visibility = None, **kwargs):
    """Thin wrapper over the rules_d d_binary rule."""
    upstream = dict(kwargs, tags = dx_upstream_tags(kwargs.get("tags", None)))
    dx_wrap_binary(name, _d_binary, _d_binary_forward, srcs, visibility = visibility, upstream_kwargs = upstream, quality_srcs = d_quality_srcs(srcs, kwargs.get("string_srcs")), **kwargs)

def d_test(name, srcs, visibility = None, **kwargs):
    """Thin wrapper over the rules_d d_test rule."""
    dx_wrap_test(name, _d_test, _d_forward_test, srcs, visibility = visibility, quality_srcs = d_quality_srcs(srcs, kwargs.get("string_srcs")), **kwargs)
