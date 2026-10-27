"""Minimal Scala wrappers."""

load("@rules_java//java:defs.bzl", "JavaInfo")
load("@rules_scala//scala:scala.bzl", _scala_binary = "scala_binary", _scala_library = "scala_library", _scala_test = "scala_test")
load("@rules_scala//scala:semanticdb_provider.bzl", "SemanticdbInfo")
load("//libs/starlark:wrapper.bzl", "DX_EXEC_PROVIDES", "dx_append_flags", "dx_executable_forward_rule", "dx_lcov_merger_attr", "dx_library_forward_rule", "dx_wrap", "dx_wrap_binary", "dx_wrap_test")
load("//quality:sources.bzl", "QualitySourcesInfo")

_DX_SCALA_LIBRARY_PROVIDES = [
    JavaInfo,
    DefaultInfo,
    InstrumentedFilesInfo,
    QualitySourcesInfo,
]

_DX_SCALA_SOURCE_SPECS = [("scala", "scala"), ("java", "java")]

_scala_library_forward = dx_library_forward_rule(
    provides = _DX_SCALA_LIBRARY_PROVIDES,
    required_providers = [(JavaInfo, "JavaInfo")],
    quality_specs = _DX_SCALA_SOURCE_SPECS,
    what = "scala_*",
    allow_files = True,
    upstream_providers = [[JavaInfo]],
    quiet_providers = [SemanticdbInfo],
)

_scala_binary_forward = dx_executable_forward_rule(
    kind = "executable",
    provides = DX_EXEC_PROVIDES,
    required_providers = [],
    quality_specs = _DX_SCALA_SOURCE_SPECS,
    what = "scala_*",
    allow_files = True,
    upstream_providers = [[JavaInfo]],
    optional_providers = [JavaInfo],
    quiet_providers = [SemanticdbInfo],
    runtime = "besteffort",
)

_scala_forward_test = dx_executable_forward_rule(
    kind = "test",
    provides = DX_EXEC_PROVIDES,
    required_providers = [],
    quality_specs = _DX_SCALA_SOURCE_SPECS,
    what = "scala_*",
    allow_files = True,
    upstream_providers = [[JavaInfo]],
    extra_attrs = dx_lcov_merger_attr(),
    optional_providers = [JavaInfo],
    quiet_providers = [SemanticdbInfo],
)

def scala_scalacopts_with_werror(kwargs):
    """Returns kwargs with -Xfatal-warnings enforced on scalacopts."""
    return dx_append_flags(kwargs, "scalacopts", ["-Xfatal-warnings"])

def scala_library(name, srcs, visibility = None, **kwargs):
    """Minimal wrapper over scala_library."""
    dx_wrap(name, _scala_library, _scala_library_forward, srcs, visibility = visibility, upstream_kwargs = scala_scalacopts_with_werror(kwargs), **kwargs)

def scala_binary(name, srcs = None, visibility = None, **kwargs):
    """Minimal wrapper over scala_binary."""
    dx_wrap_binary(name, _scala_binary, _scala_binary_forward, srcs, visibility = visibility, upstream_kwargs = scala_scalacopts_with_werror(kwargs), **kwargs)

def scala_test(name, srcs, visibility = None, **kwargs):
    """Minimal wrapper over scala_test."""
    dx_wrap_test(name, _scala_test, _scala_forward_test, srcs, visibility = visibility, upstream_kwargs = scala_scalacopts_with_werror(kwargs), **kwargs)
