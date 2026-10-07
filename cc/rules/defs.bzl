"""Experimental minimal C/C++ wrappers."""

load("@rules_cc//cc:defs.bzl", _cc_binary = "cc_binary", _cc_library = "cc_library", _cc_test = "cc_test")
load("@rules_cc//cc/common:cc_info.bzl", "CcInfo")
load("//libs/starlark:wrapper.bzl", "dx_executable_forward_rule", "dx_lcov_merger_attr", "dx_library_forward_rule", "dx_wrap", "dx_wrap_test")
load("//quality:sources.bzl", "QualitySourcesInfo")

_CC_SRCS = [".c", ".cc", ".cpp", ".cxx", ".cu"]
_CC_HDRS = [".h", ".hh", ".hpp", ".hxx", ".cuh"]

_DX_CC_LIBRARY_PROVIDES = [
    CcInfo,
    DefaultInfo,
    InstrumentedFilesInfo,
    QualitySourcesInfo,
]

_DX_CC_EXEC_PROVIDES = [
    DefaultInfo,
    QualitySourcesInfo,
]

_DX_CC_SOURCE_SPECS = [
    ("c", ["c", "h"]),
    ("cpp", ["cc", "cpp", "cxx", "hh", "hpp", "hxx"]),
    ("cuda", ["cu", "cuh"]),
]

_cc_library_forward = dx_library_forward_rule(
    provides = _DX_CC_LIBRARY_PROVIDES,
    required_providers = [(CcInfo, "CcInfo")],
    quality_specs = _DX_CC_SOURCE_SPECS,
    what = "cc_*",
    allow_files = _CC_SRCS,
    upstream_providers = [[CcInfo]],
    extra_attrs = {
        "hdrs": attr.label_list(
            allow_files = _CC_HDRS,
        ),
    },
    extra_quality_attrs = ["hdrs"],
)

_cc_binary_forward = dx_executable_forward_rule(
    kind = "executable",
    provides = _DX_CC_EXEC_PROVIDES,
    required_providers = [],
    quality_specs = _DX_CC_SOURCE_SPECS,
    what = "cc_*",
    allow_files = _CC_SRCS,
    upstream_providers = [[CcInfo]],
    extra_attrs = {
        "hdrs": attr.label_list(
            allow_files = _CC_HDRS,
        ),
    },
    extra_quality_attrs = ["hdrs"],
    optional_providers = [CcInfo],
    runtime = "besteffort",
)

_cc_forward_test = dx_executable_forward_rule(
    kind = "test",
    provides = _DX_CC_EXEC_PROVIDES,
    required_providers = [],
    quality_specs = _DX_CC_SOURCE_SPECS,
    what = "cc_*",
    allow_files = _CC_SRCS,
    upstream_providers = [[CcInfo]],
    extra_attrs = dx_lcov_merger_attr() | {
        "hdrs": attr.label_list(
            allow_files = _CC_HDRS,
        ),
    },
    extra_quality_attrs = ["hdrs"],
    optional_providers = [CcInfo],
)

def cc_library(name, srcs = None, hdrs = None, visibility = None, **kwargs):
    """Experimental minimal wrapper over cc_library."""
    effective_srcs = srcs if srcs != None else []
    effective_hdrs = hdrs if hdrs != None else []
    upstream_kwargs = dict(kwargs)
    upstream_kwargs["hdrs"] = effective_hdrs
    dx_wrap(name, _cc_library, _cc_library_forward, effective_srcs, hdrs = effective_hdrs, visibility = visibility, upstream_kwargs = upstream_kwargs, **kwargs)

def cc_binary(name, srcs, hdrs = None, visibility = None, **kwargs):
    """Experimental minimal wrapper over cc_binary."""
    forward_hdrs = hdrs if hdrs != None else []
    dx_wrap(name, _cc_binary, _cc_binary_forward, srcs, hdrs = forward_hdrs, visibility = visibility, upstream_kwargs = dict(kwargs), **kwargs)

def cc_test(name, srcs, hdrs = None, visibility = None, upstream_kwargs = None, extra_forward_kwargs = None, **kwargs):
    """Experimental minimal wrapper over cc_test."""
    base = dict(upstream_kwargs) if upstream_kwargs != None else dict(kwargs)
    forward_extra = dict(extra_forward_kwargs) if extra_forward_kwargs != None else {}
    if hdrs != None:
        forward_extra["hdrs"] = hdrs
    dx_wrap_test(name, _cc_test, _cc_forward_test, srcs, visibility = visibility, upstream_kwargs = base, extra_forward_kwargs = forward_extra, **kwargs)
