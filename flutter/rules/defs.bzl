"""Thin Flutter wrappers over rules_flutter."""

load("@rules_dart//dart:providers.bzl", "DartAnalyzableInfo", "DartInfo")
load("@rules_flutter//flutter:defs.bzl", _flutter_application = "flutter_application", _flutter_library = "flutter_library", _flutter_test = "flutter_test")
load("@rules_flutter//flutter:providers.bzl", "FlutterInfo")
load("@rules_flutter//flutter:web.bzl", _flutter_web_app = "flutter_web_app")
load("//dart/rules:defs.bzl", "DART_QUALITY_TOOLS", "DART_SOURCE_SPECS", "dart_entry_sources")
load("//libs/starlark:wrapper.bzl", "DX_EXEC_PROVIDES", "dx_executable_forward_rule", "dx_lcov_merger_attr", "dx_library_forward_rule", "dx_wrap", "dx_wrap_test")
load("//quality:sources.bzl", "QualitySourcesInfo", "QualityToolsInfo")

FLUTTER_SOURCE_EXTS = [".dart"]

FLUTTER_DEFERRED_SKIP_TAGS = []

FLUTTER_COMPATIBLE_WITH = ["@platforms//os:linux"]

FLUTTER_COVERAGE_COLLECTOR = False

_flutter_library_forward = dx_library_forward_rule(
    provides = [DartAnalyzableInfo, DartInfo, DefaultInfo, FlutterInfo, InstrumentedFilesInfo, QualitySourcesInfo, QualityToolsInfo],
    required_providers = [(DartInfo, "DartInfo"), (DartAnalyzableInfo, "DartAnalyzableInfo"), (FlutterInfo, "FlutterInfo")],
    quality_specs = DART_SOURCE_SPECS,
    quality_tools = DART_QUALITY_TOOLS,
    what = "flutter_library",
    allow_files = FLUTTER_SOURCE_EXTS,
    upstream_providers = [[DartInfo, DartAnalyzableInfo, FlutterInfo]],
    runtime = "besteffort",
)

_flutter_forward_test = dx_executable_forward_rule(
    kind = "test",
    provides = DX_EXEC_PROVIDES + [DartAnalyzableInfo, QualityToolsInfo],
    required_providers = [(DartAnalyzableInfo, "DartAnalyzableInfo")],
    quality_specs = DART_SOURCE_SPECS,
    quality_tools = DART_QUALITY_TOOLS,
    what = "flutter_test",
    allow_files = FLUTTER_SOURCE_EXTS,
    upstream_providers = [[DartAnalyzableInfo]],
    extra_attrs = dx_lcov_merger_attr(),
    runtime = "besteffort",
)

def flutter_deferred_tags(tags):
    """Adds the Flutter skip tags to a tag list."""
    out = list(tags or [])
    for tag in FLUTTER_DEFERRED_SKIP_TAGS:
        if tag not in out:
            out.append(tag)
    return out

def _flutter_kwargs(kwargs):
    return dict(
        kwargs,
        tags = flutter_deferred_tags(kwargs.get("tags")),
        target_compatible_with = (kwargs.get("target_compatible_with") or []) + FLUTTER_COMPATIBLE_WITH,
    )

def flutter_library(name, srcs = None, visibility = None, **kwargs):
    """Thin wrapper over flutter_library."""
    dx_wrap(name, _flutter_library, _flutter_library_forward, srcs or [], visibility = visibility, **_flutter_kwargs(kwargs))

def flutter_test(name, main, srcs = None, visibility = None, **kwargs):
    """Thin wrapper over flutter_test."""
    kwargs = _flutter_kwargs(kwargs)
    dx_wrap_test(name, _flutter_test, _flutter_forward_test, srcs, visibility = visibility, upstream_kwargs = dict(kwargs, main = main), quality_srcs = dart_entry_sources(main, srcs), coverage_collector = FLUTTER_COVERAGE_COLLECTOR, **kwargs)

def flutter_application(name, **kwargs):
    """Thin wrapper over flutter_application."""
    _flutter_application(name = name, **_flutter_kwargs(kwargs))

def flutter_web_app(name, **kwargs):
    """Thin wrapper over flutter_web_app."""
    _flutter_web_app(name = name, **_flutter_kwargs(kwargs))
