"""Minimal Zig wrappers."""

load("@rules_zig//zig:defs.bzl", _zig_binary = "zig_binary", _zig_library = "zig_library", _zig_test = "zig_test")
load("@rules_zig//zig/private/providers:zig_module_info.bzl", _ZigModuleInfo = "ZigModuleInfo")
load("//libs/starlark:wrapper.bzl", "DX_EXEC_PROVIDES", "dx_executable_forward_rule", "dx_lcov_merger_attr", "dx_library_forward_rule", "dx_upstream_tags", "dx_wrap", "dx_wrap_binary", "dx_wrap_test")
load("//quality:sources.bzl", "QualitySourcesInfo", "QualityToolsInfo")

# False while zig tests write empty LCOV under `bazel coverage`; the public
# test then carries dx-no-coverage-collector and `dx coverage` fails loud (#2308).
ZIG_COVERAGE_COLLECTOR = False

_DX_ZIG_LIBRARY_PROVIDES = [
    _ZigModuleInfo,
    DefaultInfo,
    InstrumentedFilesInfo,
    QualitySourcesInfo,
    QualityToolsInfo,
]

ZIG_QUALITY_TOOLS = {"zig_fmt": Label("//zig/tools:zig")}
_DX_ZIG_EXEC_PROVIDES = DX_EXEC_PROVIDES + [QualityToolsInfo]
_DX_ZIG_SOURCE_SPECS = [("zig", "zig")]
_DX_ZIG_SOURCE_EXTS = [".zig"]

_zig_library_forward = dx_library_forward_rule(
    provides = _DX_ZIG_LIBRARY_PROVIDES,
    required_providers = [(_ZigModuleInfo, "ZigModuleInfo")],
    quality_specs = _DX_ZIG_SOURCE_SPECS,
    what = "zig_*",
    allow_files = _DX_ZIG_SOURCE_EXTS,
    upstream_providers = [[_ZigModuleInfo]],
    runtime = "besteffort",
    quality_tools = ZIG_QUALITY_TOOLS,
)

def _no_coverage_impl(_settings, _attr):
    """Builds the Zig upstream without coverage instrumentation."""
    return {"//command_line_option:collect_code_coverage": False}

_no_coverage = transition(
    implementation = _no_coverage_impl,
    inputs = [],
    outputs = ["//command_line_option:collect_code_coverage"],
)

_zig_binary_forward = dx_executable_forward_rule(
    kind = "executable",
    provides = _DX_ZIG_EXEC_PROVIDES,
    required_providers = [],
    quality_specs = _DX_ZIG_SOURCE_SPECS,
    what = "zig_*",
    allow_files = _DX_ZIG_SOURCE_EXTS,
    upstream_providers = None,
    runtime = "besteffort",
    quality_tools = ZIG_QUALITY_TOOLS,
    upstream_cfg = _no_coverage,
)

_zig_forward_test = dx_executable_forward_rule(
    kind = "test",
    provides = _DX_ZIG_EXEC_PROVIDES,
    required_providers = [],
    quality_specs = _DX_ZIG_SOURCE_SPECS,
    what = "zig_*",
    allow_files = _DX_ZIG_SOURCE_EXTS,
    upstream_providers = None,
    extra_attrs = dx_lcov_merger_attr(),
    runtime = "besteffort",
    quality_tools = ZIG_QUALITY_TOOLS,
    upstream_cfg = _no_coverage,
)

def zig_quality_srcs(main, srcs):
    """Returns the hand-written sources a Zig target owns: main first, then srcs."""
    owned = [main] if main != None else []
    return owned + [s for s in (srcs or []) if s != main]

def zig_import_name(name, import_name):
    """Returns the module import name, defaulting to the public target name."""
    return import_name if import_name else name

def zig_library(name, main, srcs = None, import_name = None, visibility = None, **kwargs):
    """Minimal wrapper over zig_library; @import uses the public name."""
    dx_wrap(name, _zig_library, _zig_library_forward, srcs, visibility = visibility, quality_srcs = zig_quality_srcs(main, srcs), main = main, import_name = zig_import_name(name, import_name), **kwargs)

def zig_binary(name, main = None, srcs = None, visibility = None, **kwargs):
    """Minimal wrapper over zig_binary."""
    upstream = dict(kwargs, main = main, tags = dx_upstream_tags(kwargs.get("tags", None)))
    dx_wrap_binary(name, _zig_binary, _zig_binary_forward, srcs, visibility = visibility, upstream_kwargs = upstream, quality_srcs = zig_quality_srcs(main, srcs), main = main, **kwargs)

def zig_test(name, main = None, srcs = None, visibility = None, **kwargs):
    """Minimal wrapper over zig_test."""
    dx_wrap_test(name, _zig_test, _zig_forward_test, srcs, visibility = visibility, quality_srcs = zig_quality_srcs(main, srcs), main = main, coverage_collector = ZIG_COVERAGE_COLLECTOR, **kwargs)
