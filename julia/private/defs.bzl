"""Thin Julia wrappers over rules_julia."""

load("@rules_julia//julia:defs.bzl", _julia_binary = "julia_binary", _julia_library = "julia_library", _julia_standalone_binary = "julia_standalone_binary", _julia_test = "julia_test")
load("@rules_julia//julia:julia_info.bzl", _JuliaInfo = "JuliaInfo")
load("//core:entry.bzl", "dx_entry_quality_srcs", "dx_infer_main", "dx_main_candidates")
load("//core:fuzz_bench.bzl", "merge_tags", "with_default_size")
load("//core:platform_support.bzl", "compatible_with")
load("//core:wrapper.bzl", "DX_EXEC_PROVIDES", "dx_executable_forward_rule", "dx_forward_spec", "dx_language_kwargs", "dx_language_spec", "dx_lcov_merger_attr", "dx_library_forward_rule", "dx_private_dep_rule", "dx_upstream_tags", "dx_warnings_flags", "dx_wrap", "dx_wrap_binary", "dx_wrap_test")
load("//core/quality:sources.bzl", "QualitySourcesInfo", "QualityToolsInfo")
load("//julia/private:platforms.bzl", JULIA_SUPPORT = "PLATFORM_SUPPORT")
load("//julia/private/coverage:coverage.bzl", "dx_julia_coverage_aspect", "dx_julia_coverage_attrs", "dx_julia_test_coverage")

JuliaInfo = _JuliaInfo

JULIA_QUALITY_TOOLS = {"julia_format": Label("//julia/private/tools:julia_format")}

JULIA_SOURCE_SPECS = [("julia", "jl")]

JULIA_SOURCE_EXTS = [".jl"]

JULIA_COMPATIBLE_WITH = compatible_with(JULIA_SUPPORT)

# A deprecated call throws, and redefining a method warns.
JULIA_STRICT_FLAGS = ["--depwarn=error", "--warn-overwrite=yes"]

def julia_run_kwargs(kwargs):
    """Returns kwargs with the strict run flags and the Julia platform guard appended."""
    return dx_language_kwargs(dx_warnings_flags(kwargs, "julia_flags", JULIA_STRICT_FLAGS), compatible_with = JULIA_COMPATIBLE_WITH)

_DX_JULIA_EXEC_PROVIDES = DX_EXEC_PROVIDES + [QualityToolsInfo, _JuliaInfo]

_JULIA = dx_language_spec(what = "julia_*", quality_specs = JULIA_SOURCE_SPECS, allow_files = JULIA_SOURCE_EXTS, quality_tools = JULIA_QUALITY_TOOLS)

_julia_library_forward = dx_library_forward_rule(
    _JULIA,
    dx_forward_spec(
        provides = [_JuliaInfo, DefaultInfo, InstrumentedFilesInfo, QualitySourcesInfo, QualityToolsInfo],
        required_providers = [(_JuliaInfo, "JuliaInfo")],
        upstream_providers = [[_JuliaInfo]],
        runtime = "besteffort",
        instrument_srcs = True,
    ),
)

_julia_binary_forward = dx_executable_forward_rule(
    _JULIA,
    "executable",
    dx_forward_spec(
        provides = _DX_JULIA_EXEC_PROVIDES,
        required_providers = [(_JuliaInfo, "JuliaInfo")],
        upstream_providers = [[_JuliaInfo]],
    ),
)

_julia_forward_test = dx_executable_forward_rule(
    _JULIA,
    "test",
    dx_forward_spec(
        provides = _DX_JULIA_EXEC_PROVIDES,
        required_providers = [(_JuliaInfo, "JuliaInfo")],
        upstream_providers = [[_JuliaInfo]],
        extra_attrs = dx_lcov_merger_attr() | dx_julia_coverage_attrs(),
        upstream_aspects = [dx_julia_coverage_aspect],
        coverage_providers = dx_julia_test_coverage,
    ),
)

def _library_entry(name, srcs):
    if type(srcs) != "list":
        return None
    for candidate in [name + ".jl", "src/" + name + ".jl"]:
        if candidate in srcs or ":" + candidate in srcs:
            return candidate
    return None

def julia_library(name, srcs = None, visibility = None, **kwargs):
    """Thin wrapper over julia_library. entry defaults to name.jl, else src/name.jl, when srcs lists it."""
    kwargs = dx_language_kwargs(kwargs, compatible_with = JULIA_COMPATIBLE_WITH)
    upstream = dict(kwargs)
    if upstream.get("entry") == None:
        entry = _library_entry(name, srcs)
        if entry:
            upstream["entry"] = entry
    dx_wrap(name, _julia_library, _julia_library_forward, srcs or [], visibility = visibility, upstream_kwargs = upstream, **kwargs)

def julia_binary(name, *, srcs = None, main = None, visibility = None, **kwargs):
    """Thin wrapper over julia_binary. main is name.jl, else the single src; deprecated calls fail."""
    main = dx_infer_main("julia_binary", name, main, srcs, dx_main_candidates(name, ".jl"))
    kwargs = julia_run_kwargs(kwargs)
    upstream = dict(kwargs, main = main)
    dx_wrap_binary(name, _julia_binary, _julia_binary_forward, srcs, visibility = visibility, upstream_kwargs = upstream, quality_srcs = dx_entry_quality_srcs(main, srcs), **kwargs)

def julia_test(name, *, srcs = None, main = None, visibility = None, **kwargs):
    """Thin wrapper over julia_test. main is name.jl, else the single src; deprecated calls fail; under coverage it adds --code-coverage=user."""
    main = dx_infer_main("julia_test", name, main, srcs, dx_main_candidates(name, ".jl"))
    kwargs = julia_run_kwargs(kwargs)
    upstream = dict(kwargs)
    upstream["julia_flags"] = kwargs["julia_flags"] + select({
        Label("//core:coverage_on"): ["--code-coverage=user"],
        "//conditions:default": [],
    })
    upstream["main"] = main
    dx_wrap_test(name, _julia_test, _julia_forward_test, srcs, visibility = visibility, upstream_kwargs = upstream, quality_srcs = dx_entry_quality_srcs(main, srcs), **kwargs)

def julia_standalone_binary(name, **kwargs):
    """Standalone binary from a dx julia_library that defines julia_main()::Cint."""
    kwargs = dx_language_kwargs(kwargs, compatible_with = JULIA_COMPATIBLE_WITH)
    _julia_standalone_binary(name = name, **kwargs)

_julia_benchmark_lib = dx_private_dep_rule(
    Label("//julia/private/bench:DxBench"),
    [_JuliaInfo, DefaultInfo, InstrumentedFilesInfo, QualitySourcesInfo, QualityToolsInfo, OutputGroupInfo],
    "Provides the DxBench library julia_bench links.",
)

def julia_bench(name, *, srcs = None, main = None, deps = None, tags = None, **kwargs):
    """A julia_test run by dx bench: main calls DxBench.run_benchmarks(["name" => fn, ...]), which times each function with BenchmarkTools and writes dx-bench.json."""
    _julia_benchmark_lib(
        name = name + "_dx_bench",
        testonly = True,
        tags = dx_upstream_tags(tags),
        visibility = ["//visibility:private"],
    )
    julia_test(
        name = name,
        srcs = srcs,
        main = main,
        deps = (deps or []) + [":" + name + "_dx_bench"],
        tags = merge_tags(tags, "dx-bench"),
        **with_default_size(kwargs)
    )
