"""Experimental minimal Python wrappers."""

load("@aspect_rules_py//py:defs.bzl", _PyInfo = "PyInfo", _PyWheelsInfo = "PyWheelsInfo", _py_binary = "py_binary", _py_library = "py_library", _py_pytest_test = "py_pytest_test", _py_test = "py_test")
load("//libs/starlark:wrapper.bzl", "dx_executable_forward_rule", "dx_lcov_merger_attr", "dx_library_forward_rule", "dx_wrap", "dx_wrap_test")
load("//quality:sources.bzl", "QualitySourcesInfo")

_DX_PY_LIBRARY_PROVIDES = [
    _PyInfo,
    _PyWheelsInfo,
    DefaultInfo,
    InstrumentedFilesInfo,
    QualitySourcesInfo,
]

_DX_PY_BINARY_PROVIDES = [
    _PyInfo,
    DefaultInfo,
    InstrumentedFilesInfo,
    QualitySourcesInfo,
]

_DX_PY_SOURCE_SPECS = [("python", "py"), ("python_stub", "pyi")]
_DX_PY_SOURCE_EXTS = [".py", ".pyi"]

_python_library_forward = dx_library_forward_rule(
    provides = _DX_PY_LIBRARY_PROVIDES,
    required_providers = [(_PyInfo, "PyInfo"), (_PyWheelsInfo, "PyWheelsInfo")],
    quality_specs = _DX_PY_SOURCE_SPECS,
    what = "python_*",
    allow_files = _DX_PY_SOURCE_EXTS,
    upstream_providers = [[_PyInfo]],
)

_python_binary_forward = dx_executable_forward_rule(
    kind = "executable",
    provides = _DX_PY_BINARY_PROVIDES,
    required_providers = [(_PyInfo, "PyInfo")],
    quality_specs = _DX_PY_SOURCE_SPECS,
    what = "python_*",
    allow_files = _DX_PY_SOURCE_EXTS,
    upstream_providers = [[_PyInfo]],
)

_python_forward_test = dx_executable_forward_rule(
    kind = "test",
    provides = _DX_PY_BINARY_PROVIDES,
    required_providers = [(_PyInfo, "PyInfo")],
    quality_specs = _DX_PY_SOURCE_SPECS,
    what = "python_*",
    allow_files = _DX_PY_SOURCE_EXTS,
    upstream_providers = [[_PyInfo]],
    extra_attrs = dx_lcov_merger_attr(),
)

def _python_wrap_library(name, srcs, visibility = None, **kwargs):
    dx_wrap(name, _py_library, _python_library_forward, srcs, visibility = visibility, **kwargs)

def _python_wrap_binary(name, srcs, visibility = None, **kwargs):
    dx_wrap(name, _py_binary, _python_binary_forward, srcs, visibility = visibility, **kwargs)

def _python_wrap_py_test(name, srcs, visibility = None, **kwargs):
    dx_wrap_test(name, _py_test, _python_forward_test, srcs, visibility = visibility, **kwargs)

def python_library(name, srcs, visibility = None, **kwargs):
    """Experimental minimal wrapper over py_library."""
    _python_wrap_library(name, srcs, visibility = visibility, **kwargs)

def python_binary(name, srcs = None, main = None, visibility = None, **kwargs):
    """Experimental minimal wrapper over py_binary."""
    effective_srcs = srcs if srcs != None else []
    if main != None:
        _python_wrap_binary(name, effective_srcs, visibility = visibility, main = main, **kwargs)
    else:
        _python_wrap_binary(name, effective_srcs, visibility = visibility, **kwargs)

def python_test_rejection(kwargs):
    """Returns the rejection for forbidden python_test kwargs, or None."""
    if "main" in kwargs:
        return ("python_test always runs pytest and provides its own " +
                "entrypoint; `main` is not supported. Use python_py_test " +
                "for a custom main.")
    return None

def python_test(name, srcs, visibility = None, **kwargs):
    """Experimental minimal wrapper over py_pytest_test."""
    rejection = python_test_rejection(kwargs)
    if rejection != None:
        fail(rejection)
    dx_wrap_test(name, _py_pytest_test, _python_forward_test, srcs, visibility = visibility, **kwargs)

def python_py_test(name, srcs, main = None, visibility = None, **kwargs):
    """Experimental minimal wrapper over py_test."""
    if main != None:
        _python_wrap_py_test(name, srcs, visibility = visibility, main = main, **kwargs)
    else:
        _python_wrap_py_test(name, srcs, visibility = visibility, **kwargs)
