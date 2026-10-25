"""Minimal Java wrappers."""

load("@rules_java//java:defs.bzl", _java_binary = "java_binary", _java_library = "java_library", _java_test = "java_test")
load("@rules_java//java/common:java_info.bzl", "JavaInfo")
load("//libs/starlark:wrapper.bzl", "DX_EXEC_PROVIDES", "dx_append_flags", "dx_executable_forward_rule", "dx_lcov_merger_attr", "dx_library_forward_rule", "dx_wrap", "dx_wrap_binary", "dx_wrap_test")
load("//quality:sources.bzl", "QualitySourcesInfo")

_DX_JAVA_LIBRARY_PROVIDES = [
    JavaInfo,
    DefaultInfo,
    InstrumentedFilesInfo,
    QualitySourcesInfo,
]

_DX_JAVA_SOURCE_SPECS = [("java", "java")]
_DX_JAVA_SOURCE_EXTS = [".java"]

_java_library_forward = dx_library_forward_rule(
    provides = _DX_JAVA_LIBRARY_PROVIDES,
    required_providers = [(JavaInfo, "JavaInfo")],
    quality_specs = _DX_JAVA_SOURCE_SPECS,
    what = "java_*",
    allow_files = _DX_JAVA_SOURCE_EXTS,
    upstream_providers = [[JavaInfo]],
)

_java_binary_forward = dx_executable_forward_rule(
    kind = "executable",
    provides = DX_EXEC_PROVIDES,
    required_providers = [],
    quality_specs = _DX_JAVA_SOURCE_SPECS,
    what = "java_*",
    allow_files = _DX_JAVA_SOURCE_EXTS,
    upstream_providers = [[JavaInfo]],
    optional_providers = [JavaInfo],
    runtime = "besteffort",
)

_java_forward_test = dx_executable_forward_rule(
    kind = "test",
    provides = DX_EXEC_PROVIDES,
    required_providers = [],
    quality_specs = _DX_JAVA_SOURCE_SPECS,
    what = "java_*",
    allow_files = _DX_JAVA_SOURCE_EXTS,
    upstream_providers = [[JavaInfo]],
    extra_attrs = dx_lcov_merger_attr(),
    optional_providers = [JavaInfo],
)

def java_javacopts_with_werror(kwargs):
    """Returns kwargs with -Werror plus -Xlint:all enforced on javacopts."""
    return dx_append_flags(kwargs, "javacopts", ["-Werror", "-Xlint:all"])

def java_library(name, srcs, visibility = None, **kwargs):
    """Minimal wrapper over java_library."""
    dx_wrap(name, _java_library, _java_library_forward, srcs, visibility = visibility, upstream_kwargs = java_javacopts_with_werror(kwargs), **kwargs)

def java_binary(name, srcs = None, visibility = None, **kwargs):
    """Minimal wrapper over java_binary."""
    dx_wrap_binary(name, _java_binary, _java_binary_forward, srcs, visibility = visibility, upstream_kwargs = java_javacopts_with_werror(kwargs), **kwargs)

def java_test(name, srcs, visibility = None, **kwargs):
    """Minimal wrapper over java_test."""
    dx_wrap_test(name, _java_test, _java_forward_test, srcs, visibility = visibility, upstream_kwargs = java_javacopts_with_werror(kwargs), **kwargs)
