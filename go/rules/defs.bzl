"""Minimal Go wrappers."""

load("@rules_go//go:def.bzl", _GoArchive = "GoArchive", _GoInfo = "GoInfo", _go_binary = "go_binary", _go_library = "go_library", _go_test = "go_test")
load("//libs/starlark:wrapper.bzl", "DX_EXEC_PROVIDES", "dx_executable_forward_rule", "dx_lcov_merger_attr", "dx_library_forward_rule", "dx_wrap", "dx_wrap_binary", "dx_wrap_test")
load("//quality:sources.bzl", "QualitySourcesInfo", "UPSTREAM_CC_CLASS_EXTENSIONS")

_DX_GO_LIBRARY_PROVIDES = [
    _GoInfo,
    _GoArchive,
    DefaultInfo,
    InstrumentedFilesInfo,
    QualitySourcesInfo,
]

_DX_GO_SOURCE_SPECS = [("go", "go")] + [(k, UPSTREAM_CC_CLASS_EXTENSIONS[k]) for k in ("c", "cpp")]

_go_library_forward = dx_library_forward_rule(
    provides = _DX_GO_LIBRARY_PROVIDES,
    required_providers = [(_GoInfo, "GoInfo"), (_GoArchive, "GoArchive")],
    quality_specs = _DX_GO_SOURCE_SPECS,
    what = "go_*",
    allow_files = True,
    upstream_providers = [[_GoInfo]],
)

_go_binary_forward = dx_executable_forward_rule(
    kind = "executable",
    provides = DX_EXEC_PROVIDES,
    required_providers = [],
    quality_specs = _DX_GO_SOURCE_SPECS,
    what = "go_*",
    allow_files = True,
    upstream_providers = [[_GoArchive]],
    optional_providers = [_GoArchive],
    runtime = "besteffort",
)

_go_forward_test = dx_executable_forward_rule(
    kind = "test",
    provides = DX_EXEC_PROVIDES,
    required_providers = [],
    quality_specs = _DX_GO_SOURCE_SPECS,
    what = "go_*",
    allow_files = True,
    upstream_providers = [[_GoArchive]],
    extra_attrs = dx_lcov_merger_attr(),
    optional_providers = [_GoArchive],
)

def go_library(name, srcs, importpath, visibility = None, **kwargs):
    """Minimal wrapper over go_library."""
    dx_wrap(name, _go_library, _go_library_forward, srcs, visibility = visibility, importpath = importpath, **kwargs)

def go_binary(name, srcs = None, visibility = None, **kwargs):
    """Minimal wrapper over go_binary."""
    dx_wrap_binary(name, _go_binary, _go_binary_forward, srcs, visibility = visibility, **kwargs)

def go_test(name, srcs = None, visibility = None, **kwargs):
    """Minimal wrapper over go_test."""
    dx_wrap_test(name, _go_test, _go_forward_test, srcs, visibility = visibility, **kwargs)
