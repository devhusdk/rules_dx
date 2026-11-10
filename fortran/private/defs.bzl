"""Thin Fortran wrappers over rules_fortran, linked by the C++ toolchain."""

load("@rules_cc//cc:defs.bzl", _cc_binary = "cc_binary", _cc_test = "cc_test")
load("@rules_cc//cc/common:cc_info.bzl", "CcInfo")
load("@rules_fortran//fortran:defs.bzl", "FortranInfo", _fortran_library = "fortran_library")
load("//libs/starlark:wrapper.bzl", "DX_EXEC_PROVIDES", "dx_executable_forward_rule", "dx_lcov_merger_attr", "dx_library_forward_rule", "dx_wrap", "dx_wrap_binary", "dx_wrap_test")
load("//quality:sources.bzl", "QualitySourcesInfo")
load("//tools/coverage/kcov:kcov.bzl", "dx_kcov_attrs", "dx_kcov_test_coverage")
load(":kwargs.bzl", _FORTRAN_COMPATIBLE_WITH = "FORTRAN_COMPATIBLE_WITH", "fortran_library_kwargs", "fortran_link_kwargs", "fortran_with_defaults")

FORTRAN_COMPATIBLE_WITH = _FORTRAN_COMPATIBLE_WITH

FORTRAN_EXTENSIONS = ["f", "f90", "f95", "f03", "f08", "F", "F90", "F95", "F03", "F08"]

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
    instrument_srcs = True,
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

def _kcov_upstream_impl(settings, _attr):
    """Builds the test upstream for kcov under coverage: debug info, unstripped, uninstrumented."""
    coverage = settings["//command_line_option:collect_code_coverage"]
    return {
        "//command_line_option:collect_code_coverage": False,
        "//command_line_option:strip": "never" if coverage else settings["//command_line_option:strip"],
        "//fortran/private:kcov": coverage,
    }

_kcov_upstream = transition(
    implementation = _kcov_upstream_impl,
    inputs = [
        "//command_line_option:collect_code_coverage",
        "//command_line_option:strip",
    ],
    outputs = [
        "//command_line_option:collect_code_coverage",
        "//command_line_option:strip",
        "//fortran/private:kcov",
    ],
)

def _fortran_test_coverage(ctx, upstream):
    """Runs the upstream Fortran test under kcov on Linux."""
    deps = [dep[FortranInfo].transitive_sources for dep in ctx.attr.deps if FortranInfo in dep]
    sources = [f for f in depset(ctx.files.srcs, transitive = deps).to_list() if "." + f.extension in _SOURCE_EXTS]
    return dx_kcov_test_coverage(ctx, upstream, sources, ["upstream", "deps"])

_fortran_forward_test = dx_executable_forward_rule(
    kind = "test",
    provides = DX_EXEC_PROVIDES,
    required_providers = [],
    quality_specs = _FORTRAN,
    what = "fortran_*",
    allow_files = _SOURCE_EXTS,
    upstream_providers = None,
    extra_attrs = dx_lcov_merger_attr() | dx_kcov_attrs() | {
        "deps": attr.label_list(doc = "Dependencies of the wrapped test, whose sources count for coverage."),
    },
    runtime = "besteffort",
    upstream_cfg = _kcov_upstream,
    coverage_providers = _fortran_test_coverage,
)

def library_name(name):
    """Returns the name of the private Fortran library behind one executable."""
    return name + "_fortran"

def _linked(cc_rule):
    def _macro(name, visibility = None, **kwargs):
        lib = fortran_library_kwargs(kwargs)
        _fortran_library(name = library_name(name), visibility = ["//visibility:private"], **lib)
        cc_rule(name = name, visibility = visibility, deps = [":" + library_name(name)], **fortran_link_kwargs(kwargs))

    return _macro

_fortran_cc_binary = _linked(_cc_binary)

_fortran_cc_test = _linked(_cc_test)

def fortran_library(name, srcs = None, visibility = None, **kwargs):
    """Thin wrapper over the rules_fortran fortran_library rule."""
    dx_wrap(name, _fortran_library, _library_forward, srcs or [], visibility = visibility, **fortran_with_defaults(kwargs))

def fortran_binary(name, *, srcs = None, visibility = None, **kwargs):
    """Compiles Fortran sources with rules_fortran and links them with cc_binary."""
    dx_wrap_binary(name, _fortran_cc_binary, _binary_forward, srcs or [], visibility = visibility, **fortran_with_defaults(kwargs))

def fortran_test(name, *, srcs = None, visibility = None, **kwargs):
    """Compiles Fortran sources with rules_fortran and links them with cc_test."""
    dx_wrap_test(name, _fortran_cc_test, _fortran_forward_test, srcs or [], visibility = visibility, extra_forward_kwargs = {"deps": kwargs.get("deps", [])}, **fortran_with_defaults(kwargs))
