"""Experimental minimal Kotlin wrappers."""

load("@rules_java//java:defs.bzl", "JavaInfo")
load("@rules_kotlin//kotlin:jvm.bzl", _kt_jvm_binary = "kt_jvm_binary", _kt_jvm_library = "kt_jvm_library", _kt_jvm_test = "kt_jvm_test")
load("//libs/starlark:wrapper.bzl", "dx_executable_forward_rule", "dx_lcov_merger_attr", "dx_library_forward_rule", "dx_wrap", "dx_wrap_binary", "dx_wrap_test")
load("//quality:sources.bzl", "QualitySourcesInfo")

_KT_TOOLCHAIN_TYPE = "@rules_kotlin//kotlin/internal:kt_toolchain_type"

def _kotlin_coverage_runfiles(ctx):
    """Returns the Jacoco runner runfiles the upstream launcher needs under coverage."""
    if not ctx.configuration.coverage_enabled:
        return None
    runner = ctx.toolchains[_KT_TOOLCHAIN_TYPE].jacocorunner
    if runner == None:
        return None
    return ctx.runfiles(transitive_files = depset(runner.files.to_list()))

_DX_KOTLIN_LIBRARY_PROVIDES = [
    JavaInfo,
    DefaultInfo,
    InstrumentedFilesInfo,
    QualitySourcesInfo,
]

_DX_KOTLIN_EXEC_PROVIDES = [
    DefaultInfo,
    QualitySourcesInfo,
]

_DX_KOTLIN_SOURCE_SPECS = [("kotlin", ["kt", "kts"])]
_DX_KOTLIN_SOURCE_EXTS = [".kt", ".kts"]

_kotlin_library_forward = dx_library_forward_rule(
    provides = _DX_KOTLIN_LIBRARY_PROVIDES,
    required_providers = [(JavaInfo, "JavaInfo")],
    quality_specs = _DX_KOTLIN_SOURCE_SPECS,
    what = "kotlin_*",
    allow_files = _DX_KOTLIN_SOURCE_EXTS,
    upstream_providers = [[JavaInfo]],
)

_kotlin_binary_forward = dx_executable_forward_rule(
    kind = "executable",
    provides = _DX_KOTLIN_EXEC_PROVIDES,
    required_providers = [],
    quality_specs = _DX_KOTLIN_SOURCE_SPECS,
    what = "kotlin_*",
    allow_files = _DX_KOTLIN_SOURCE_EXTS,
    upstream_providers = [[JavaInfo]],
    optional_providers = [JavaInfo],
    runtime = "besteffort",
    coverage_runfiles = _kotlin_coverage_runfiles,
    toolchains = [_KT_TOOLCHAIN_TYPE],
)

_kotlin_forward_test = dx_executable_forward_rule(
    kind = "test",
    provides = _DX_KOTLIN_EXEC_PROVIDES,
    required_providers = [],
    quality_specs = _DX_KOTLIN_SOURCE_SPECS,
    what = "kotlin_*",
    allow_files = _DX_KOTLIN_SOURCE_EXTS,
    upstream_providers = [[JavaInfo]],
    extra_attrs = dx_lcov_merger_attr(),
    optional_providers = [JavaInfo],
)

def kotlin_kotlinc_opts_with_werror(kwargs):
    """Returns kwargs defaulting kotlinc_opts to warnings_as_errors."""
    upstream_kwargs = dict(kwargs)
    upstream_kwargs.setdefault("kotlinc_opts", "//kotlin/rules:warnings_as_errors")
    return upstream_kwargs

def _kotlin_with_werror(kwargs):
    return kotlin_kotlinc_opts_with_werror(kwargs)

def _kotlin_wrap_library(name, srcs, visibility = None, **kwargs):
    dx_wrap(name, _kt_jvm_library, _kotlin_library_forward, srcs, visibility = visibility, **_kotlin_with_werror(kwargs))

def _kotlin_wrap_binary(name, srcs, visibility = None, **kwargs):
    dx_wrap_binary(name, _kt_jvm_binary, _kotlin_binary_forward, srcs, visibility = visibility, upstream_kwargs = _kotlin_with_werror(kwargs), **kwargs)

def kotlin_library(name, srcs, visibility = None, **kwargs):
    """Experimental minimal wrapper over kt_jvm_library."""
    _kotlin_wrap_library(name, srcs, visibility = visibility, **kwargs)

def kotlin_binary(name, srcs = None, main_class = None, visibility = None, **kwargs):
    """Experimental minimal wrapper over kt_jvm_binary."""
    effective_srcs = srcs if srcs != None else []
    upstream_kwargs = dict(kwargs)
    if main_class != None:
        upstream_kwargs["main_class"] = main_class
    _kotlin_wrap_binary(name, effective_srcs, visibility = visibility, **upstream_kwargs)

def kotlin_test(name, srcs, visibility = None, **kwargs):
    """Experimental minimal wrapper over kt_jvm_test."""
    dx_wrap_test(name, _kt_jvm_test, _kotlin_forward_test, srcs, visibility = visibility, upstream_kwargs = _kotlin_with_werror(kwargs), **kwargs)
