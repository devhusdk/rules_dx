"""Minimal Kotlin wrappers."""

load("@rules_java//java:defs.bzl", "JavaInfo")
load("@rules_kotlin//kotlin:jvm.bzl", _kt_jvm_binary = "kt_jvm_binary", _kt_jvm_library = "kt_jvm_library", _kt_jvm_test = "kt_jvm_test")
load("//libs/starlark:wrapper.bzl", "DX_EXEC_PROVIDES", "dx_executable_forward_rule", "dx_lcov_merger_attr", "dx_library_forward_rule", "dx_setdefaults", "dx_wrap", "dx_wrap_binary", "dx_wrap_test")
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
    provides = DX_EXEC_PROVIDES,
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
    provides = DX_EXEC_PROVIDES,
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
    return dx_setdefaults(kwargs, {"kotlinc_opts": str(Label("//kotlin/rules:warnings_as_errors"))})

def kotlin_library(name, srcs, visibility = None, **kwargs):
    """Minimal wrapper over kt_jvm_library."""
    dx_wrap(name, _kt_jvm_library, _kotlin_library_forward, srcs, visibility = visibility, upstream_kwargs = kotlin_kotlinc_opts_with_werror(kwargs), **kwargs)

def kotlin_binary(name, srcs = None, visibility = None, **kwargs):
    """Minimal wrapper over kt_jvm_binary."""
    dx_wrap_binary(name, _kt_jvm_binary, _kotlin_binary_forward, srcs, visibility = visibility, upstream_kwargs = kotlin_kotlinc_opts_with_werror(kwargs), **kwargs)

def kotlin_test(name, srcs, visibility = None, **kwargs):
    """Minimal wrapper over kt_jvm_test."""
    dx_wrap_test(name, _kt_jvm_test, _kotlin_forward_test, srcs, visibility = visibility, upstream_kwargs = kotlin_kotlinc_opts_with_werror(kwargs), **kwargs)
