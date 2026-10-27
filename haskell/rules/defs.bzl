"""Thin Haskell wrappers over rules_haskell."""

load("@rules_cc//cc/common:cc_info.bzl", "CcInfo")
load("@rules_haskell//haskell:defs.bzl", _haskell_binary = "haskell_binary", _haskell_library = "haskell_library", _haskell_test = "haskell_test")
load("@rules_haskell//haskell:providers.bzl", _HaskellCoverageInfo = "HaskellCoverageInfo", _HaskellInfo = "HaskellInfo", _HaskellLibraryInfo = "HaskellLibraryInfo")
load("//haskell/toolchain:pins.bzl", "CELLS")
load("//libs/starlark:cells.bzl", "cells_compatible_with")
load("//libs/starlark:wrapper.bzl", "dx_executable_forward_rule", "dx_library_forward_rule", "dx_upstream_tags", "dx_wrap_binary", "dx_wrap_test")
load("//quality:sources.bzl", "QualitySourcesInfo")

HaskellInfo = _HaskellInfo

HaskellLibraryInfo = _HaskellLibraryInfo

HASKELL_COVERAGE_COLLECTOR = False

HASKELL_CELLS = sorted(CELLS.keys())

HASKELL_COMPATIBLE_WITH = cells_compatible_with(CELLS, "//haskell/toolchain", HASKELL_CELLS)

DEFAULT_GHCOPTS = ["-Wall", "-Werror"]

GMP = Label("@dx_haskell_toolchains//:gmp")

_HASKELL = [("haskell", ["hs"])]

_SOURCES = [".hs", ".lhs", ".hsc", ".hs-boot", ".lhs-boot"]

_haskell_library_forward = dx_library_forward_rule(
    provides = [_HaskellInfo, _HaskellLibraryInfo, CcInfo, DefaultInfo, InstrumentedFilesInfo, QualitySourcesInfo],
    required_providers = [(_HaskellInfo, "HaskellInfo"), (_HaskellLibraryInfo, "HaskellLibraryInfo"), (CcInfo, "CcInfo")],
    quality_specs = _HASKELL,
    what = "haskell_library",
    allow_files = _SOURCES,
    upstream_providers = [[_HaskellInfo, _HaskellLibraryInfo]],
    quiet_providers = [_HaskellCoverageInfo],
    runtime = "besteffort",
)

_haskell_forward_binary = dx_executable_forward_rule(
    kind = "executable",
    provides = [DefaultInfo, QualitySourcesInfo],
    required_providers = [],
    quality_specs = _HASKELL,
    what = "haskell_binary",
    allow_files = _SOURCES,
    upstream_providers = None,
    quiet_providers = [_HaskellInfo],
    runtime = "besteffort",
)

_haskell_forward_test = dx_executable_forward_rule(
    kind = "test",
    provides = [DefaultInfo, QualitySourcesInfo],
    required_providers = [],
    quality_specs = _HASKELL,
    what = "haskell_test",
    allow_files = _SOURCES,
    upstream_providers = None,
    quiet_providers = [_HaskellInfo],
    runtime = "besteffort",
)

def upstream_kwargs(kwargs):
    """Returns the rules_haskell kwargs with the dx ghcopts default filled in."""
    upstream = dict(kwargs)
    if upstream.get("ghcopts") == None:
        upstream["ghcopts"] = list(DEFAULT_GHCOPTS)
    return upstream

def with_gmp(deps, gmp = GMP):
    """Returns executable deps with the GMP runtime that ghc-bignum links added once."""
    if type(deps) == "select":
        return deps + [gmp]
    deps = list(deps or [])
    if gmp not in deps and str(gmp) not in deps:
        deps.append(gmp)
    return deps

def _forward_kwargs(kwargs, testonly):
    out = {"testonly": True} if testonly else {}
    for key in ("tags", "target_compatible_with"):
        if kwargs.get(key) != None:
            out[key] = kwargs[key]
    return out

def haskell_library(name, srcs = [], visibility = None, testonly = False, **kwargs):
    """Thin wrapper over the rules_haskell haskell_library macro."""
    upstream = upstream_kwargs(kwargs)
    upstream["tags"] = dx_upstream_tags(upstream.get("tags"))
    _haskell_library(name = name + "_upstream", srcs = srcs, testonly = testonly, visibility = ["//visibility:private"], **upstream)
    _haskell_library_forward(name = name, upstream = ":" + name + "_upstream", srcs = srcs, visibility = visibility, **_forward_kwargs(kwargs, testonly))

def haskell_binary(name, srcs = [], visibility = None, **kwargs):
    """Thin wrapper over the rules_haskell haskell_binary macro."""
    upstream = upstream_kwargs(kwargs)
    upstream["deps"] = with_gmp(upstream.get("deps"))
    dx_wrap_binary(name, _haskell_binary, _haskell_forward_binary, srcs, visibility = visibility, upstream_kwargs = upstream, **kwargs)

def haskell_test(name, srcs = [], visibility = None, **kwargs):
    """Thin wrapper over the rules_haskell haskell_test macro."""
    upstream = upstream_kwargs(kwargs)
    upstream["deps"] = with_gmp(upstream.get("deps"))
    dx_wrap_test(name, _haskell_test, _haskell_forward_test, srcs, visibility = visibility, upstream_kwargs = upstream, coverage_collector = HASKELL_COVERAGE_COLLECTOR, **kwargs)
