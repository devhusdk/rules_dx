"""Minimal Python wrappers."""

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

def _python_main_matches(name, srcs):
    """Returns the srcs that end in the wrapper basename plus .py."""
    proposed = name.split("/")[-1] + ".py"
    return [src for src in srcs if str(src) == proposed or str(src).endswith(":" + proposed) or str(src).endswith("/" + proposed)]

def python_main(name, srcs):
    """Returns the src py_binary picks as main for the wrapper name, or None."""
    if type(srcs) != "list" or len(srcs) <= 1:
        return None
    matches = _python_main_matches(name, srcs)
    return matches[0] if len(matches) == 1 else None

def python_main_rejection(name, srcs):
    """Returns the rejection for multiple srcs without one main match, or None."""
    if type(srcs) != "list" or len(srcs) <= 1:
        return None
    proposed = name.split("/")[-1] + ".py"
    matches = _python_main_matches(name, srcs)
    if not matches:
        return "has multiple srcs and none is {}; set main.".format(proposed)
    if len(matches) > 1:
        return "has multiple srcs matching {}: {}; set main.".format(proposed, ", ".join([str(m) for m in matches]))
    return None

def _python_main_kwargs(kind, name, srcs, kwargs):
    """Returns upstream kwargs with main set from the wrapper name, or None."""
    if kwargs.get("main") != None:
        return None
    rejection = python_main_rejection(name, srcs)
    if rejection != None:
        fail("{} //{}:{} {}".format(kind, native.package_name(), name, rejection))
    main = python_main(name, srcs)
    if main == None:
        return None
    return dict(kwargs, main = main)

def python_library(name, srcs, visibility = None, **kwargs):
    """Minimal wrapper over py_library."""
    dx_wrap(name, _py_library, _python_library_forward, srcs, visibility = visibility, **kwargs)

def python_binary(name, srcs = None, visibility = None, **kwargs):
    """Minimal wrapper over py_binary."""
    srcs = srcs if srcs != None else []
    upstream_kwargs = _python_main_kwargs("python_binary", name, srcs, kwargs)
    dx_wrap(name, _py_binary, _python_binary_forward, srcs, visibility = visibility, upstream_kwargs = upstream_kwargs, executable = True, **kwargs)

def python_test_rejection(kwargs):
    """Returns the rejection for forbidden python_test kwargs, or None."""
    if "main" in kwargs:
        return ("python_test always runs pytest and provides its own " +
                "entrypoint; `main` is not supported. Use python_py_test " +
                "for a custom main.")
    return None

def python_test(name, srcs, visibility = None, **kwargs):
    """Minimal wrapper over py_pytest_test."""
    rejection = python_test_rejection(kwargs)
    if rejection != None:
        fail(rejection)
    dx_wrap_test(name, _py_pytest_test, _python_forward_test, srcs, visibility = visibility, **kwargs)

def python_py_test(name, srcs, visibility = None, **kwargs):
    """Minimal wrapper over py_test."""
    upstream_kwargs = _python_main_kwargs("python_py_test", name, srcs, kwargs)
    dx_wrap_test(name, _py_test, _python_forward_test, srcs, visibility = visibility, upstream_kwargs = upstream_kwargs, **kwargs)
