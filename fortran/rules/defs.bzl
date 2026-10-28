"""Thin Fortran wrappers over rules_fortran, linked by the C++ toolchain."""

load("@rules_cc//cc:defs.bzl", _cc_binary = "cc_binary", _cc_test = "cc_test")
load("@rules_cc//cc/common:cc_info.bzl", "CcInfo")
load("@rules_fortran//fortran:defs.bzl", "FortranInfo", _fortran_library = "fortran_library")
load("//libs/starlark:wrapper.bzl", "DX_EXEC_PROVIDES", "dx_executable_forward_rule", "dx_lcov_merger_attr", "dx_library_forward_rule", "dx_wrap", "dx_wrap_binary", "dx_wrap_test")
load("//quality:sources.bzl", "QualitySourcesInfo")

FORTRAN_COMPATIBLE_WITH = [
    Label("@platforms//os:linux"),
    Label("@platforms//cpu:x86_64"),
]

SKIP_TAGS = ["no-format", "no-typecheck"]

FORTRAN_EXTENSIONS = ["f", "f90", "f95", "f03", "f08", "F", "F90", "F95", "F03", "F08"]

LIBRARY_ATTRS = ["copts", "defines", "deps", "hdrs", "includes", "srcs"]

SHARED_ATTRS = ["tags", "target_compatible_with", "testonly"]

_FORTRAN = [("fortran", FORTRAN_EXTENSIONS)]

_SOURCE_EXTS = ["." + ext for ext in FORTRAN_EXTENSIONS]

_library_forward = dx_library_forward_rule(
    provides = [FortranInfo, CcInfo, DefaultInfo, InstrumentedFilesInfo, QualitySourcesInfo],
    required_providers = [(FortranInfo, "FortranInfo"), (CcInfo, "CcInfo")],
    quality_specs = _FORTRAN,
    what = "fortran_*",
    allow_files = _SOURCE_EXTS,
    upstream_providers = [[FortranInfo]],
    runtime = "besteffort",
)

_binary_forward = dx_executable_forward_rule(
    kind = "executable",
    provides = DX_EXEC_PROVIDES,
    required_providers = [],
    quality_specs = _FORTRAN,
    what = "fortran_*",
    allow_files = _SOURCE_EXTS,
    upstream_providers = None,
    runtime = "besteffort",
)

_fortran_forward_test = dx_executable_forward_rule(
    kind = "test",
    provides = DX_EXEC_PROVIDES,
    required_providers = [],
    quality_specs = _FORTRAN,
    what = "fortran_*",
    allow_files = _SOURCE_EXTS,
    upstream_providers = None,
    extra_attrs = dx_lcov_merger_attr(),
    runtime = "besteffort",
)

def with_defaults(kwargs):
    """Returns kwargs with the qualified Fortran cell and the quality skip tags as defaults."""
    out = dict(kwargs)
    if out.get("target_compatible_with") == None:
        out["target_compatible_with"] = FORTRAN_COMPATIBLE_WITH
    tags = list(out.get("tags") or [])
    out["tags"] = tags + [tag for tag in SKIP_TAGS if tag not in tags]
    return out

def library_kwargs(kwargs):
    """Returns the attributes of an executable that the Fortran library compiles with."""
    return {key: value for key, value in kwargs.items() if key in LIBRARY_ATTRS or key in SHARED_ATTRS}

def link_kwargs(kwargs):
    """Returns the attributes of an executable that the C++ link rule takes."""
    return {key: value for key, value in kwargs.items() if key not in LIBRARY_ATTRS}

def library_name(name):
    """Returns the name of the private Fortran library behind one executable."""
    return name + "_fortran"

def _linked(cc_rule):
    def _macro(name, visibility = None, **kwargs):
        lib = library_kwargs(kwargs)
        _fortran_library(name = library_name(name), visibility = ["//visibility:private"], **lib)
        cc_rule(name = name, visibility = visibility, deps = [":" + library_name(name)], **link_kwargs(kwargs))

    return _macro

_fortran_cc_binary = _linked(_cc_binary)

_fortran_cc_test = _linked(_cc_test)

def fortran_library(name, srcs = None, visibility = None, **kwargs):
    """Thin wrapper over the rules_fortran fortran_library rule."""
    dx_wrap(name, _fortran_library, _library_forward, srcs or [], visibility = visibility, **with_defaults(kwargs))

def fortran_binary(name, srcs = None, visibility = None, **kwargs):
    """Compiles Fortran sources with rules_fortran and links them with cc_binary."""
    dx_wrap_binary(name, _fortran_cc_binary, _binary_forward, srcs or [], visibility = visibility, **with_defaults(kwargs))

def fortran_test(name, srcs = None, visibility = None, **kwargs):
    """Compiles Fortran sources with rules_fortran and links them with cc_test."""
    dx_wrap_test(name, _fortran_cc_test, _fortran_forward_test, srcs or [], visibility = visibility, coverage_collector = False, **with_defaults(kwargs))
