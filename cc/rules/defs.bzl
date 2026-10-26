"""Minimal C/C++ wrappers."""

load("@rules_cc//cc:defs.bzl", _cc_binary = "cc_binary", _cc_library = "cc_library", _cc_test = "cc_test")
load("@rules_cc//cc/common:cc_info.bzl", "CcInfo")
load("//libs/starlark:wrapper.bzl", "DX_EXEC_PROVIDES", "dx_executable_forward_rule", "dx_lcov_merger_attr", "dx_library_forward_rule", "dx_wrap", "dx_wrap_test")
load("//quality:sources.bzl", "QualitySourcesInfo")

_CC_SRCS = [".c", ".cc", ".cpp", ".cxx"]
_CC_HDRS = [".h", ".hh", ".hpp", ".hxx"]

_DX_CC_LIBRARY_PROVIDES = [
    CcInfo,
    DefaultInfo,
    InstrumentedFilesInfo,
    QualitySourcesInfo,
]

_DX_CC_SOURCE_SPECS = [
    ("c", ["c", "h"]),
    ("cpp", ["cc", "cpp", "cxx", "hh", "hpp", "hxx"]),
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
    provides = DX_EXEC_PROVIDES,
    required_providers = [],
    quality_specs = _DX_CC_SOURCE_SPECS,
    what = "cc_*",
    allow_files = _CC_SRCS,
    upstream_providers = [[CcInfo]],
    optional_providers = [CcInfo],
    runtime = "besteffort",
)

_cc_forward_test = dx_executable_forward_rule(
    kind = "test",
    provides = DX_EXEC_PROVIDES,
    required_providers = [],
    quality_specs = _DX_CC_SOURCE_SPECS,
    what = "cc_*",
    allow_files = _CC_SRCS,
    upstream_providers = [[CcInfo]],
    extra_attrs = dx_lcov_merger_attr(),
    optional_providers = [CcInfo],
)

def cc_library(name, srcs = None, hdrs = None, visibility = None, **kwargs):
    """Minimal wrapper over cc_library."""
    effective_srcs = srcs if srcs != None else []
    effective_hdrs = hdrs if hdrs != None else []
    dx_wrap(name, _cc_library, _cc_library_forward, effective_srcs, visibility = visibility, hdrs = effective_hdrs, extra_forward_kwargs = {"hdrs": effective_hdrs}, **kwargs)

def cc_binary(name, srcs, visibility = None, **kwargs):
    """Minimal wrapper over cc_binary."""
    dx_wrap(name, _cc_binary, _cc_binary_forward, srcs, visibility = visibility, executable = True, **kwargs)

def cc_test(name, srcs, visibility = None, **kwargs):
    """Minimal wrapper over cc_test."""
    dx_wrap_test(name, _cc_test, _cc_forward_test, srcs, visibility = visibility, **kwargs)
