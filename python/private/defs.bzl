"""Minimal Python wrappers."""

load("@bazel_skylib//rules:native_binary.bzl", "native_binary")
load("@rules_py//py:defs.bzl", _PyInfo = "PyInfo", _PyWheelsInfo = "PyWheelsInfo", _py_binary = "py_binary", _py_library = "py_library", _py_pytest_test = "py_pytest_test", _py_test = "py_test")
load("//core:entry.bzl", "dx_entry_quality_srcs")
load("//core:fuzz.bzl", "dx_fuzz_test")
load("//core:fuzz_bench.bzl", "merge_tags", "with_default_size")
load("//core:wrapper.bzl", "dx_executable_forward_rule", "dx_forward_spec", "dx_language_spec", "dx_lcov_merger_attr", "dx_library_forward_rule", "dx_private_dep_rule", "dx_upstream_tags", "dx_warnings_flags", "dx_wrap", "dx_wrap_binary", "dx_wrap_test")
load("//core/quality:sources.bzl", "QualitySourcesInfo")

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

_PYTHON = dx_language_spec(what = "python_*", quality_specs = _DX_PY_SOURCE_SPECS, allow_files = _DX_PY_SOURCE_EXTS)

_python_library_forward = dx_library_forward_rule(
    _PYTHON,
    dx_forward_spec(
        provides = _DX_PY_LIBRARY_PROVIDES,
        required_providers = [(_PyInfo, "PyInfo"), (_PyWheelsInfo, "PyWheelsInfo")],
        upstream_providers = [[_PyInfo]],
    ),
)

_python_binary_forward = dx_executable_forward_rule(
    _PYTHON,
    "executable",
    dx_forward_spec(
        provides = _DX_PY_BINARY_PROVIDES,
        required_providers = [(_PyInfo, "PyInfo")],
        upstream_providers = [[_PyInfo]],
        runtime = "mandatory",
    ),
)

_python_forward_test = dx_executable_forward_rule(
    _PYTHON,
    "test",
    dx_forward_spec(
        provides = _DX_PY_BINARY_PROVIDES,
        required_providers = [(_PyInfo, "PyInfo")],
        upstream_providers = [[_PyInfo]],
        extra_attrs = dx_lcov_merger_attr(),
        runtime = "mandatory",
    ),
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
        return "has multiple srcs ({}) and none is {}; set main.".format(", ".join([str(s) for s in srcs]), proposed)
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

def python_library(name, srcs = None, visibility = None, **kwargs):
    """Minimal wrapper over py_library."""
    dx_wrap(name, _py_library, _python_library_forward, srcs, visibility = visibility, **kwargs)

def python_binary(name, *, srcs = None, main = None, visibility = None, **kwargs):
    """Wrapper over py_binary that infers main."""
    if main != None:
        kwargs["main"] = main
    srcs = srcs if srcs != None else []
    upstream_kwargs = _python_main_kwargs("python_binary", name, srcs, kwargs)
    dx_wrap_binary(name, _py_binary, _python_binary_forward, srcs, visibility = visibility, upstream_kwargs = upstream_kwargs, quality_srcs = dx_entry_quality_srcs(main if main != None else python_main(name, srcs), srcs), **kwargs)

def python_test_rejection(runner, kwargs):
    """Returns the rejection for a bad python_test runner or kwargs, or None."""
    if runner not in ("pytest", "plain"):
        return "python_test runner must be \"pytest\" or \"plain\", got %r." % (runner,)
    if runner == "pytest" and "main" in kwargs:
        return "python_test with runner = \"pytest\" runs pytest; set runner = \"plain\" to use main."
    return None

_python_coverage_sitecustomize = dx_private_dep_rule(
    Label("//python/private/coverage:sitecustomize"),
    _DX_PY_LIBRARY_PROVIDES,
    "Provides the sitecustomize python_test loads to collect coverage.",
)

_python_runner_plugin = dx_private_dep_rule(
    Label("//python/private:bench_plugin"),
    _DX_PY_LIBRARY_PROVIDES,
    "Provides the pytest plugin python_bench loads.",
)

def python_test(name, *, srcs = None, main = None, runner = "pytest", visibility = None, **kwargs):
    """Wrapper over py_pytest_test, or py_test for the plain runner. The interpreter runs with -Werror unless warnings_as_errors is False."""
    if main != None:
        kwargs["main"] = main
    rejection = python_test_rejection(runner, kwargs)
    if rejection != None:
        fail(rejection)
    _python_coverage_sitecustomize(
        name = name + "_sitecustomize",
        testonly = True,
        tags = dx_upstream_tags(kwargs.get("tags")),
        visibility = ["//visibility:private"],
    )
    kwargs["deps"] = kwargs.get("deps", []) + [":" + name + "_sitecustomize"]
    kwargs = dx_warnings_flags(kwargs, "interpreter_options", ["-Werror"])
    if runner == "plain":
        upstream_kwargs = _python_main_kwargs("python_test", name, srcs, kwargs)
        dx_wrap_test(name, _py_test, _python_forward_test, srcs, visibility = visibility, upstream_kwargs = upstream_kwargs, quality_srcs = dx_entry_quality_srcs(main if main != None else python_main(name, srcs), srcs), **kwargs)
    else:
        dx_wrap_test(name, _py_pytest_test, _python_forward_test, srcs, visibility = visibility, **kwargs)

def python_fuzz_test(name, *, srcs = None, visibility = None, tags = None, **kwargs):
    """An atheris fuzz binary plus a runner for dx fuzz."""
    kwargs = dict(kwargs)
    linux_x86_64 = ["@platforms//os:linux", "@platforms//cpu:x86_64"]
    compatible = linux_x86_64 + [c for c in kwargs.pop("target_compatible_with", []) if c not in linux_x86_64]
    dx_fuzz_test(
        name,
        python_binary,
        {"srcs": srcs, "target_compatible_with": compatible},
        tags = tags,
        visibility = visibility,
        fuzz_rule = native_binary,
        fuzz_kwargs = {"src": ":" + name + "_bin", "out": name + "_fuzz", "target_compatible_with": compatible},
        **kwargs
    )

def python_bench(name, *, srcs = None, args = None, tags = None, deps = None, **kwargs):
    """python_test run by dx bench: pytest-benchmark numbers land in dx-bench.json."""
    _python_runner_plugin(
        name = name + "_bench_plugin",
        testonly = True,
        tags = dx_upstream_tags(tags),
        visibility = ["//visibility:private"],
    )
    python_test(
        name = name,
        srcs = srcs,
        args = ["-p", "dx_bench_plugin", "--benchmark-only"] + (args or []),
        tags = merge_tags(tags, "dx-bench"),
        deps = (deps or []) + [":" + name + "_bench_plugin"],
        **with_default_size(kwargs)
    )
