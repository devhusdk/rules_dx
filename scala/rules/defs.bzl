"""Minimal Scala wrappers."""

load("@rules_java//java:defs.bzl", "JavaInfo")
load("@rules_scala//scala:scala.bzl", _scala_binary = "scala_binary", _scala_library = "scala_library", _scala_test = "scala_test")
load("@rules_scala//scala:semanticdb_provider.bzl", "SemanticdbInfo")
load("//libs/starlark:wrapper.bzl", "dx_executable_forward_rule", "dx_lcov_merger_attr", "dx_library_forward_rule", "dx_wrap", "dx_wrap_binary", "dx_wrap_test")
load("//quality:sources.bzl", "QualitySourcesInfo")

_DX_SCALA_LIBRARY_PROVIDES = [
    JavaInfo,
    DefaultInfo,
    InstrumentedFilesInfo,
    QualitySourcesInfo,
]

_DX_SCALA_EXEC_PROVIDES = [
    DefaultInfo,
    QualitySourcesInfo,
]

_DX_SCALA_SOURCE_SPECS = [("scala", "scala")]
_DX_SCALA_SOURCE_EXTS = [".scala"]

_scala_library_forward = dx_library_forward_rule(
    provides = _DX_SCALA_LIBRARY_PROVIDES,
    required_providers = [(JavaInfo, "JavaInfo")],
    quality_specs = _DX_SCALA_SOURCE_SPECS,
    what = "scala_*",
    allow_files = _DX_SCALA_SOURCE_EXTS,
    upstream_providers = [[JavaInfo]],
    quiet_providers = [SemanticdbInfo],
)

_scala_binary_forward = dx_executable_forward_rule(
    kind = "executable",
    provides = _DX_SCALA_EXEC_PROVIDES,
    required_providers = [],
    quality_specs = _DX_SCALA_SOURCE_SPECS,
    what = "scala_*",
    allow_files = _DX_SCALA_SOURCE_EXTS,
    upstream_providers = [[JavaInfo]],
    optional_providers = [JavaInfo],
    quiet_providers = [SemanticdbInfo],
    runtime = "besteffort",
)

_scala_forward_test = dx_executable_forward_rule(
    kind = "test",
    provides = _DX_SCALA_EXEC_PROVIDES,
    required_providers = [],
    quality_specs = _DX_SCALA_SOURCE_SPECS,
    what = "scala_*",
    allow_files = _DX_SCALA_SOURCE_EXTS,
    upstream_providers = [[JavaInfo]],
    extra_attrs = dx_lcov_merger_attr(),
    optional_providers = [JavaInfo],
    quiet_providers = [SemanticdbInfo],
)

def scala_scalacopts_with_werror(kwargs):
    """Returns kwargs with -Xfatal-warnings enforced on scalacopts."""
    upstream_kwargs = dict(kwargs)
    scalacopts = list(upstream_kwargs.get("scalacopts", []))
    if "-Xfatal-warnings" not in scalacopts:
        scalacopts = scalacopts + ["-Xfatal-warnings"]
    upstream_kwargs["scalacopts"] = scalacopts
    return upstream_kwargs

def _scala_with_werror(kwargs):
    return scala_scalacopts_with_werror(kwargs)

def _scala_wrap_library(name, srcs, visibility = None, **kwargs):
    dx_wrap(name, _scala_library, _scala_library_forward, srcs, visibility = visibility, **_scala_with_werror(kwargs))

def _scala_wrap_binary(name, srcs, visibility = None, **kwargs):
    dx_wrap_binary(name, _scala_binary, _scala_binary_forward, srcs, visibility = visibility, upstream_kwargs = _scala_with_werror(kwargs), **kwargs)

def scala_library(name, srcs, visibility = None, **kwargs):
    """Minimal wrapper over scala_library."""
    _scala_wrap_library(name, srcs, visibility = visibility, **kwargs)

def scala_binary(name, srcs = None, main_class = None, visibility = None, **kwargs):
    """Minimal wrapper over scala_binary."""
    effective_srcs = srcs if srcs != None else []
    upstream_kwargs = dict(kwargs)
    if main_class != None:
        upstream_kwargs["main_class"] = main_class
    _scala_wrap_binary(name, effective_srcs, visibility = visibility, **upstream_kwargs)

def scala_test(name, srcs, visibility = None, **kwargs):
    """Minimal wrapper over scala_test."""
    dx_wrap_test(name, _scala_test, _scala_forward_test, srcs, visibility = visibility, upstream_kwargs = _scala_with_werror(kwargs), **kwargs)
