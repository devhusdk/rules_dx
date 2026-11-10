"""Thin Dart wrappers over rules_dart."""

load("@rules_dart//dart:defs.bzl", _dart_binary = "dart_binary", _dart_codegen = "dart_codegen", _dart_js_binary = "dart_js_binary", _dart_library = "dart_library", _dart_test = "dart_test", _dart_wasm_binary = "dart_wasm_binary")
load("@rules_dart//dart:providers.bzl", "DartAnalyzableInfo", "DartCompileInfo", "DartInfo")
load("//dart/private:dart_config.bzl", _DxDartConfigInfo = "DxDartConfigInfo", _dart_config = "dart_config")
load("//core:entry.bzl", "dx_entry_quality_srcs", "dx_infer_main", "dx_main_candidates")
load("//core:fuzz_bench.bzl", "merge_tags", "with_default_size")
load("//core:platform_support.bzl", "compatible_with")
load("//core:wrapper.bzl", "DX_EXEC_PROVIDES", "dx_executable_forward_rule", "dx_language_kwargs", "dx_lcov_merger_attr", "dx_library_forward_rule", "dx_without_warnings_attr", "dx_wrap", "dx_wrap_binary", "dx_wrap_test")
load("//dart/private:platforms.bzl", DART_SUPPORT = "PLATFORM_SUPPORT")
load("//quality:sources.bzl", "QualitySourcesInfo", "QualityToolsInfo")

DxDartConfigInfo = _DxDartConfigInfo

dart_config = _dart_config

DART_SOURCE_SPECS = [("dart", "dart")]

DART_SOURCE_EXTS = [".dart"]

DART_COMPATIBLE_WITH = compatible_with(DART_SUPPORT)

DART_DEFERRED_SKIP_TAGS = []

DART_QUALITY_TOOLS = {"dart": Label("//dart/private/tools:dart")}

_dart_library_forward = dx_library_forward_rule(
    provides = [DartInfo, DefaultInfo, InstrumentedFilesInfo, QualitySourcesInfo, QualityToolsInfo],
    required_providers = [(DartInfo, "DartInfo")],
    quality_specs = DART_SOURCE_SPECS,
    quality_tools = DART_QUALITY_TOOLS,
    what = "dart_library",
    allow_files = DART_SOURCE_EXTS,
    upstream_providers = [[DartInfo]],
    runtime = "besteffort",
)

_dart_binary_forward = dx_executable_forward_rule(
    kind = "executable",
    provides = DX_EXEC_PROVIDES + [DartAnalyzableInfo, DartCompileInfo, QualityToolsInfo],
    required_providers = [(DartAnalyzableInfo, "DartAnalyzableInfo"), (DartCompileInfo, "DartCompileInfo")],
    quality_specs = DART_SOURCE_SPECS,
    quality_tools = DART_QUALITY_TOOLS,
    what = "dart_binary",
    allow_files = DART_SOURCE_EXTS,
    upstream_providers = [[DartAnalyzableInfo, DartCompileInfo]],
    runtime = "besteffort",
)

_dart_web_forward = dx_library_forward_rule(
    provides = [DartAnalyzableInfo, DefaultInfo, InstrumentedFilesInfo, QualitySourcesInfo, QualityToolsInfo],
    required_providers = [(DartAnalyzableInfo, "DartAnalyzableInfo")],
    quality_specs = DART_SOURCE_SPECS,
    quality_tools = DART_QUALITY_TOOLS,
    what = "dart web binary",
    allow_files = DART_SOURCE_EXTS,
    upstream_providers = [[DartAnalyzableInfo]],
    runtime = "besteffort",
)

_dart_forward_test = dx_executable_forward_rule(
    kind = "test",
    provides = DX_EXEC_PROVIDES + [DartAnalyzableInfo, QualityToolsInfo],
    required_providers = [(DartAnalyzableInfo, "DartAnalyzableInfo")],
    quality_specs = DART_SOURCE_SPECS,
    quality_tools = DART_QUALITY_TOOLS,
    what = "dart_test",
    allow_files = DART_SOURCE_EXTS,
    upstream_providers = [[DartAnalyzableInfo]],
    extra_attrs = dx_lcov_merger_attr(),
    runtime = "besteffort",
)

# Re-exported unchanged: BUILD files need it to generate Dart sources.
dart_codegen = _dart_codegen

def dart_root_package_name(package, name, kwargs):
    """Returns the package name a root-package library needs, or None."""
    if package != "" or "package_name" in kwargs or "package" in kwargs:
        return None
    return name

def dart_library(name, srcs = None, visibility = None, **kwargs):
    """Thin wrapper over dart_library."""
    kwargs = dx_without_warnings_attr(kwargs)
    kwargs = dx_language_kwargs(kwargs, DART_DEFERRED_SKIP_TAGS, DART_COMPATIBLE_WITH)
    root_name = dart_root_package_name(native.package_name(), name, kwargs)
    if root_name != None:
        kwargs["package_name"] = root_name
    dx_wrap(name, _dart_library, _dart_library_forward, srcs or [], visibility = visibility, **kwargs)

def dart_binary(name, *, main = None, srcs = None, visibility = None, **kwargs):
    """Thin wrapper over dart_binary. main is name.dart, then bin/name.dart, else the single src."""
    kwargs = dx_without_warnings_attr(kwargs)
    main = dx_infer_main("dart_binary", name, main, srcs, dx_main_candidates(name, ".dart", ["bin"]))
    kwargs = dx_language_kwargs(kwargs, DART_DEFERRED_SKIP_TAGS, DART_COMPATIBLE_WITH)
    dx_wrap_binary(name, _dart_binary, _dart_binary_forward, srcs, visibility = visibility, upstream_kwargs = dict(kwargs, main = main), quality_srcs = dx_entry_quality_srcs(main, srcs), **kwargs)

def _dart_web_binary(kind, upstream_rule, name, main, srcs, visibility, kwargs):
    kwargs = dx_without_warnings_attr(kwargs)
    main = dx_infer_main(kind, name, main, srcs, dx_main_candidates(name, ".dart", ["bin"]))
    kwargs = dx_language_kwargs(kwargs, DART_DEFERRED_SKIP_TAGS, DART_COMPATIBLE_WITH)
    dx_wrap_binary(name, upstream_rule, _dart_web_forward, srcs, visibility = visibility, upstream_kwargs = dict(kwargs, main = main), quality_srcs = dx_entry_quality_srcs(main, srcs), **kwargs)

def dart_js_binary(name, *, main = None, srcs = None, visibility = None, **kwargs):
    """Thin wrapper over dart_js_binary. main is inferred as for dart_binary."""
    _dart_web_binary("dart_js_binary", _dart_js_binary, name, main, srcs, visibility, kwargs)

def dart_wasm_binary(name, *, main = None, srcs = None, visibility = None, **kwargs):
    """Thin wrapper over dart_wasm_binary. main is inferred as for dart_binary."""
    _dart_web_binary("dart_wasm_binary", _dart_wasm_binary, name, main, srcs, visibility, kwargs)

def dart_test(name, *, main = None, srcs = None, visibility = None, **kwargs):
    """Thin wrapper over dart_test. main is name.dart, then test/name.dart, else the single src."""
    kwargs = dx_without_warnings_attr(kwargs)
    main = dx_infer_main("dart_test", name, main, srcs, dx_main_candidates(name, ".dart", ["test"]))
    kwargs = dx_language_kwargs(kwargs, DART_DEFERRED_SKIP_TAGS, DART_COMPATIBLE_WITH)
    dx_wrap_test(name, _dart_test, _dart_forward_test, srcs, visibility = visibility, upstream_kwargs = dict(kwargs, main = main), quality_srcs = dx_entry_quality_srcs(main, srcs), **kwargs)

def dart_bench(name, *, main = None, srcs = None, deps = None, tags = None, **kwargs):
    """A dart_test run by dx bench: `main` calls runBenchmarks from package:dx_bench with BenchmarkBase objects, which writes dx-bench.json."""
    dart_test(
        name = name,
        main = main,
        srcs = srcs,
        deps = (deps or []) + [Label("//dart/private/bench")],
        tags = merge_tags(tags, "dx-bench", "manual"),
        **with_default_size(kwargs)
    )
