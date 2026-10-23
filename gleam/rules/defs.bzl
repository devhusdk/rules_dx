"""Thin Gleam wrappers over rules_gleam."""

load("@rules_gleam//gleam:defs.bzl", _gleam_binary = "gleam_binary", _gleam_erl_library = "gleam_erl_library", _gleam_library = "gleam_library", _gleam_test = "gleam_test")
load("@rules_gleam//gleam:provider.bzl", _GleamErlPackageInfo = "GleamErlPackageInfo")
load("//erlang/rules:defs.bzl", "ERLANG_COMPATIBLE_WITH")
load("//libs/starlark:wrapper.bzl", "dx_executable_forward_rule", "dx_lcov_merger_attr", "dx_library_forward_rule", "dx_wrap_binary", "dx_wrap_test")
load("//quality:sources.bzl", "QualitySourcesInfo")
load(":javascript.bzl", _GleamJsInfo = "GleamJsInfo", _gleam_js_binary = "gleam_js_binary", _gleam_js_library = "gleam_js_library", _gleam_js_test = "gleam_js_test", _package_name = "package_name")

GleamErlPackageInfo = _GleamErlPackageInfo

GleamJsInfo = _GleamJsInfo

GLEAM_COMPATIBLE_WITH = ERLANG_COMPATIBLE_WITH

GLEAM_SOURCE_GLOBS = ["src/**/*.gleam"]

GLEAM_TEST_GLOBS = ["test/**/*_test.gleam"]

GLEAM_JS_SOURCE_GLOBS = ["src/**/*.gleam", "src/**/*.mjs"]

GLEEUNIT = Label("//gleam/lib:gleeunit")

GLEEUNIT_JS = Label("//gleam/lib:gleeunit_js")

_GLEAM = [("gleam", ["gleam"])]

_ERLANG = [("erlang", ["erl"])]

_library_forward = dx_library_forward_rule(
    provides = [_GleamErlPackageInfo, DefaultInfo, InstrumentedFilesInfo, QualitySourcesInfo],
    required_providers = [(_GleamErlPackageInfo, "GleamErlPackageInfo")],
    quality_specs = _GLEAM,
    what = "gleam_library",
    allow_files = [".gleam"],
    upstream_providers = [[_GleamErlPackageInfo]],
    runtime = "besteffort",
)

_erl_library_forward = dx_library_forward_rule(
    provides = [_GleamErlPackageInfo, DefaultInfo, InstrumentedFilesInfo, QualitySourcesInfo],
    required_providers = [(_GleamErlPackageInfo, "GleamErlPackageInfo")],
    quality_specs = _ERLANG,
    what = "gleam_erl_library",
    allow_files = [".erl"],
    upstream_providers = [[_GleamErlPackageInfo]],
    runtime = "besteffort",
)

_js_library_forward = dx_library_forward_rule(
    provides = [_GleamJsInfo, DefaultInfo, InstrumentedFilesInfo, QualitySourcesInfo],
    required_providers = [(_GleamJsInfo, "GleamJsInfo")],
    quality_specs = _GLEAM,
    what = "gleam_js_library",
    allow_files = [".gleam", ".mjs"],
    upstream_providers = [[_GleamJsInfo]],
    runtime = "besteffort",
)

_gleam_js_forward_binary = dx_executable_forward_rule(
    kind = "executable",
    provides = [DefaultInfo, QualitySourcesInfo],
    required_providers = [],
    quality_specs = _GLEAM,
    what = "gleam_js_binary",
    allow_files = [".gleam", ".mjs"],
    upstream_providers = None,
    quiet_providers = [_GleamJsInfo],
    runtime = "besteffort",
)

_gleam_js_forward_test = dx_executable_forward_rule(
    kind = "test",
    provides = [DefaultInfo, QualitySourcesInfo],
    required_providers = [],
    quality_specs = _GLEAM,
    what = "gleam_js_test",
    allow_files = [".gleam", ".mjs"],
    upstream_providers = None,
    extra_attrs = dx_lcov_merger_attr(),
    runtime = "besteffort",
)

_gleam_forward_binary = dx_executable_forward_rule(
    kind = "executable",
    provides = [DefaultInfo, QualitySourcesInfo],
    required_providers = [],
    quality_specs = _GLEAM,
    what = "gleam_binary",
    allow_files = [".gleam"],
    upstream_providers = None,
    quiet_providers = [_GleamErlPackageInfo],
    runtime = "besteffort",
)

_gleam_forward_test = dx_executable_forward_rule(
    kind = "test",
    provides = [DefaultInfo, QualitySourcesInfo],
    required_providers = [],
    quality_specs = _GLEAM,
    what = "gleam_test",
    allow_files = [".gleam"],
    upstream_providers = None,
    extra_attrs = dx_lcov_merger_attr(),
    runtime = "besteffort",
)

def source_root(package, directory):
    """Returns the strip_src_prefix that makes paths below one package directory module names."""
    return package + "/" + directory if package else directory

def owned_sources(srcs, globs, glob = None):
    """Returns the explicit srcs, or the files the default globs own."""
    if srcs != None:
        return srcs
    glob = glob if glob != None else native.glob
    return glob(globs, allow_empty = True)

def upstream_kwargs(kwargs, srcs, directory, package = None):
    """Returns the rules_gleam kwargs with dx defaults filled in."""
    upstream = dict(kwargs)
    upstream["srcs"] = srcs
    if upstream.get("strip_src_prefix") == None:
        upstream["strip_src_prefix"] = source_root(package if package != None else native.package_name(), directory)
    if upstream.get("warnings_as_errors") == None:
        upstream["warnings_as_errors"] = True
    return upstream

def js_kwargs(kwargs, srcs, directory, name):
    """Returns the JavaScript rule kwargs with dx defaults and a package named after the target."""
    upstream = upstream_kwargs(kwargs, srcs, directory)
    if upstream.get("package") == None:
        upstream["package"] = _package_name(native.repo_name(), native.package_name(), name)
    return upstream

def with_gleeunit(deps, gleeunit = GLEEUNIT):
    """Returns test deps with the pinned gleeunit runner added once."""
    deps = list(deps or [])
    if gleeunit not in deps and str(gleeunit) not in deps:
        deps.append(gleeunit)
    return deps

def _forward_kwargs(kwargs, testonly):
    out = {"testonly": True} if testonly else {}
    for key in ("tags", "target_compatible_with"):
        if kwargs.get(key) != None:
            out[key] = kwargs[key]
    return out

def gleam_library(name, srcs = None, visibility = None, testonly = False, **kwargs):
    """Thin wrapper over the rules_gleam gleam_library rule."""
    srcs = owned_sources(srcs, GLEAM_SOURCE_GLOBS)
    _gleam_library(name = name + "_upstream", testonly = testonly, visibility = ["//visibility:private"], **upstream_kwargs(kwargs, srcs, "src"))
    _library_forward(name = name, upstream = ":" + name + "_upstream", srcs = srcs, visibility = visibility, **_forward_kwargs(kwargs, testonly))

def gleam_erl_library(name, srcs = None, visibility = None, testonly = False, **kwargs):
    """Thin wrapper over the rules_gleam gleam_erl_library rule."""
    srcs = owned_sources(srcs, ["src/**/*.erl"])
    _gleam_erl_library(name = name + "_upstream", testonly = testonly, visibility = ["//visibility:private"], **upstream_kwargs(kwargs, srcs, "src"))
    _erl_library_forward(name = name, upstream = ":" + name + "_upstream", srcs = srcs, visibility = visibility, **_forward_kwargs(kwargs, testonly))

def gleam_binary(name, srcs, visibility = None, **kwargs):
    """Thin wrapper over the rules_gleam gleam_binary rule."""
    upstream = upstream_kwargs(kwargs, srcs, "src")
    upstream.pop("srcs")
    dx_wrap_binary(name, _gleam_binary, _gleam_forward_binary, srcs, visibility = visibility, upstream_kwargs = upstream, **kwargs)

def gleam_test(name, srcs = None, visibility = None, **kwargs):
    """Thin wrapper over the rules_gleam gleam_test macro with the pinned gleeunit."""
    srcs = owned_sources(srcs, GLEAM_TEST_GLOBS)
    upstream = upstream_kwargs(kwargs, srcs, "test")
    upstream["deps"] = with_gleeunit(upstream.get("deps"))
    upstream.pop("srcs")
    dx_wrap_test(name, _gleam_test, _gleam_forward_test, srcs, visibility = visibility, upstream_kwargs = upstream, **kwargs)

def gleam_js_library(name, srcs = None, visibility = None, testonly = False, **kwargs):
    """Compiles one Gleam package and its .mjs FFI files to JavaScript."""
    srcs = owned_sources(srcs, GLEAM_JS_SOURCE_GLOBS)
    _gleam_js_library(name = name + "_upstream", testonly = testonly, visibility = ["//visibility:private"], **js_kwargs(kwargs, srcs, "src", name))
    _js_library_forward(name = name, upstream = ":" + name + "_upstream", srcs = srcs, visibility = visibility, **_forward_kwargs(kwargs, testonly))

def gleam_js_binary(name, srcs, visibility = None, **kwargs):
    """Runs one Gleam main function on the managed Node.js runtime."""
    upstream = js_kwargs(kwargs, srcs, "src", name)
    upstream.pop("srcs")
    dx_wrap_binary(name, _gleam_js_binary, _gleam_js_forward_binary, srcs, visibility = visibility, upstream_kwargs = upstream, **kwargs)

def gleam_js_test(name, srcs = None, visibility = None, **kwargs):
    """Runs gleeunit tests on the managed Node.js runtime with the pinned gleeunit."""
    srcs = owned_sources(srcs, GLEAM_TEST_GLOBS)
    upstream = js_kwargs(kwargs, srcs, "test", name)
    upstream["deps"] = with_gleeunit(upstream.get("deps"), GLEEUNIT_JS)
    upstream.pop("srcs")
    dx_wrap_test(name, _gleam_js_test, _gleam_js_forward_test, srcs, visibility = visibility, upstream_kwargs = upstream, **kwargs)
