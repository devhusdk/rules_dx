"""Thin Haskell wrappers over rules_haskell."""

load("@rules_cc//cc/common:cc_info.bzl", "CcInfo")
load("@rules_haskell//haskell:defs.bzl", _haskell_binary = "haskell_binary", _haskell_library = "haskell_library", _haskell_test = "haskell_test")
load("@rules_haskell//haskell:providers.bzl", _HaskellCoverageInfo = "HaskellCoverageInfo", _HaskellInfo = "HaskellInfo", _HaskellLibraryInfo = "HaskellLibraryInfo")
load("@rules_shell//shell:sh_test.bzl", "sh_test")
load("//haskell/toolchain:transition.bzl", "ghc_mingw_transition")
load("//libs/starlark:wrapper.bzl", "dx_executable_forward_rule", "dx_lcov_merger_attr", "dx_library_forward_rule", "dx_upstream_tags", "dx_wrap_binary", "dx_wrap_test")
load("//libs/starlark:fuzz_bench.bzl", "merge_tags", "split_test_kwargs", "with_default_size")
load("//quality:sources.bzl", "QualitySourcesInfo")
load("//tools/coverage/haskell:coverage.bzl", "dx_haskell_coverage_attrs", "dx_haskell_test_coverage")
load(":kwargs.bzl", _DEFAULT_GHCOPTS = "DEFAULT_GHCOPTS", _HASKELL_CELLS = "HASKELL_CELLS", _HASKELL_COMPATIBLE_WITH = "HASKELL_COMPATIBLE_WITH", _STANDARD_GHCOPTS = "STANDARD_GHCOPTS", _STRICT_GHCOPTS = "STRICT_GHCOPTS", "haskell_kwargs", "haskell_upstream_kwargs")

DEFAULT_GHCOPTS = _DEFAULT_GHCOPTS
HASKELL_CELLS = _HASKELL_CELLS
HASKELL_COMPATIBLE_WITH = _HASKELL_COMPATIBLE_WITH
STANDARD_GHCOPTS = _STANDARD_GHCOPTS
STRICT_GHCOPTS = _STRICT_GHCOPTS

HaskellInfo = _HaskellInfo

HaskellLibraryInfo = _HaskellLibraryInfo

GMP = Label("@dx_haskell_toolchains//:gmp")

_HASKELL = [("haskell", ["hs"])]

_SOURCES = [".hs", ".lhs", ".hsc", ".hs-boot", ".lhs-boot"]

_haskell_library_forward = dx_library_forward_rule(
    provides = [_HaskellInfo, _HaskellLibraryInfo, CcInfo, DefaultInfo, InstrumentedFilesInfo, QualitySourcesInfo],
    required_providers = [(_HaskellInfo, "HaskellInfo"), (_HaskellLibraryInfo, "HaskellLibraryInfo"), (CcInfo, "CcInfo")],
    quality_specs = _HASKELL,
    what = "haskell_library",
    allow_files = _SOURCES,
    upstream_providers = [[_HaskellInfo, _HaskellLibraryInfo]],
    quiet_providers = [_HaskellCoverageInfo],
    runtime = "besteffort",
    upstream_cfg = ghc_mingw_transition,
)

_haskell_forward_binary = dx_executable_forward_rule(
    kind = "executable",
    provides = [DefaultInfo, QualitySourcesInfo],
    required_providers = [],
    quality_specs = _HASKELL,
    what = "haskell_binary",
    allow_files = _SOURCES,
    upstream_providers = None,
    quiet_providers = [_HaskellInfo],
    runtime = "besteffort",
    upstream_cfg = ghc_mingw_transition,
)

_haskell_forward_test = dx_executable_forward_rule(
    kind = "test",
    provides = [DefaultInfo, QualitySourcesInfo],
    required_providers = [],
    quality_specs = _HASKELL,
    what = "haskell_test",
    allow_files = _SOURCES,
    upstream_providers = None,
    extra_attrs = dx_lcov_merger_attr() | dx_haskell_coverage_attrs(),
    quiet_providers = [_HaskellInfo],
    runtime = "besteffort",
    coverage_providers = dx_haskell_test_coverage,
    upstream_cfg = ghc_mingw_transition,
)

def with_gmp(deps, gmp = GMP):
    """Returns executable deps with the GMP runtime that ghc-bignum links added once."""
    if type(deps) == "select":
        return deps + [gmp]
    deps = list(deps or [])
    if gmp not in deps and str(gmp) not in deps:
        deps.append(gmp)
    return deps

def _forward_kwargs(kwargs, testonly):
    out = {"testonly": True} if testonly else {}
    for key in ("tags", "target_compatible_with"):
        if kwargs.get(key) != None:
            out[key] = kwargs[key]
    return out

def haskell_library(name, srcs = [], visibility = None, testonly = False, **kwargs):
    """Thin wrapper over the rules_haskell haskell_library macro."""
    kwargs = haskell_kwargs(kwargs)
    upstream = haskell_upstream_kwargs(kwargs)
    upstream["tags"] = dx_upstream_tags(upstream.get("tags"))
    _haskell_library(name = name + "_upstream", srcs = srcs, testonly = testonly, visibility = ["//visibility:private"], **upstream)
    _haskell_library_forward(name = name, upstream = ":" + name + "_upstream", srcs = srcs, visibility = visibility, **_forward_kwargs(kwargs, testonly))

def haskell_binary(name, srcs = [], visibility = None, **kwargs):
    """Thin wrapper over the rules_haskell haskell_binary macro."""
    kwargs = haskell_kwargs(kwargs)
    upstream = haskell_upstream_kwargs(kwargs)
    upstream["deps"] = with_gmp(upstream.get("deps"))
    dx_wrap_binary(name, _haskell_binary, _haskell_forward_binary, srcs, visibility = visibility, upstream_kwargs = upstream, **kwargs)

def haskell_test(name, srcs = [], visibility = None, **kwargs):
    """Thin wrapper over the rules_haskell haskell_test macro."""
    kwargs = haskell_kwargs(kwargs)
    upstream = haskell_upstream_kwargs(kwargs)
    upstream["deps"] = with_gmp(upstream.get("deps"))
    dx_wrap_test(name, _haskell_test, _haskell_forward_test, srcs, visibility = visibility, upstream_kwargs = upstream, **kwargs)

def haskell_bench(name, srcs = [], visibility = None, tags = None, **kwargs):
    """A haskell_binary of tasty-bench benchmarks plus a `<name>` runner that writes dx-bench.json for `dx bench`."""
    binary = name + "_bin"
    build, run = split_test_kwargs(kwargs)
    run = with_default_size(run)
    haskell_binary(
        name = binary,
        srcs = srcs,
        visibility = ["//visibility:private"],
        tags = merge_tags(tags, "manual"),
        testonly = True,
        **build
    )
    sh_test(
        name = name,
        srcs = [Label("//haskell/rules:bench_run.sh")],
        args = ["$(rootpath :" + binary + ")"] + run.pop("args", []),
        data = run.pop("data", []) + [":" + binary],
        tags = merge_tags(tags, "dx-bench", "manual"),
        testonly = True,
        visibility = visibility,
        **run
    )
