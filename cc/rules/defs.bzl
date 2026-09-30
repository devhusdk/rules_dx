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
    extra_attrs = dx_lcov_merger_attr(),
    optional_providers = [CcInfo],
)

def cc_copts_with_werror(kwargs):
    """Returns kwargs with warnings-as-errors enforced on copts."""
    upstream_kwargs = dict(kwargs)
    copts = list(upstream_kwargs.get("copts", []))
    if "-Werror" in copts or "/WX" in copts:
        return upstream_kwargs

    def _msvc_opt(flag):
        if flag.startswith("-std=c++") or flag.startswith("-std=gnu++"):
            if "14" in flag:
                return "/std:c++14"
            if "20" in flag:
                return "/std:c++20"
            return "/std:c++17"
        return flag

    win_copts = [_msvc_opt(c) for c in copts] + ["/Zc:__cplusplus", "/WX"]
    upstream_kwargs["copts"] = select({
        "@platforms//os:windows": win_copts,
        "@platforms//os:macos": copts + ["-Werror", "-fno-profile-instr-generate", "-fno-coverage-mapping"],
        "//conditions:default": copts + ["-Werror"],
    })
    return upstream_kwargs

def _cc_with_werror(kwargs):
    return cc_copts_with_werror(kwargs)

def _cc_wrap_library(name, srcs, hdrs, visibility = None, **kwargs):
    dx_wrap(name, _cc_library, _cc_library_forward, srcs, hdrs = hdrs, visibility = visibility, **_cc_with_werror(kwargs))

def _cc_wrap_binary(name, srcs, visibility = None, **kwargs):
    dx_wrap(name, _cc_binary, _cc_binary_forward, srcs, visibility = visibility, **_cc_with_werror(kwargs))

def cc_library(name, srcs = None, hdrs = None, visibility = None, **kwargs):
    """Experimental minimal wrapper over cc_library."""
    effective_srcs = srcs if srcs != None else []
    effective_hdrs = hdrs if hdrs != None else []
    _cc_wrap_library(name, effective_srcs, effective_hdrs, visibility = visibility, **kwargs)

def cc_binary(name, srcs, visibility = None, **kwargs):
    """Experimental minimal wrapper over cc_binary."""
    _cc_wrap_binary(name, srcs, visibility = visibility, **kwargs)

def cc_test(name, srcs, visibility = None, **kwargs):
    """Experimental minimal wrapper over cc_test."""
    dx_wrap_test(name, _cc_test, _cc_forward_test, srcs, visibility = visibility, upstream_kwargs = _cc_with_werror(kwargs), **kwargs)
