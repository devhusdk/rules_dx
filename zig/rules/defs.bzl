"""Minimal Zig wrappers."""

load("@rules_zig//zig:defs.bzl", _zig_binary = "zig_binary", _zig_library = "zig_library", _zig_test = "zig_test")
load("@rules_zig//zig/private/providers:zig_module_info.bzl", _ZigModuleInfo = "ZigModuleInfo")
load("//libs/starlark:wrapper.bzl", "dx_binary_forward_kwargs", "dx_binary_upstream_kwargs", "dx_executable_forward_rule", "dx_lcov_merger_attr", "dx_library_forward_rule", "dx_test_forward_kwargs", "dx_test_upstream_kwargs", "dx_upstream_tags")
load("//quality:sources.bzl", "QualitySourcesInfo")

_DX_ZIG_LIBRARY_PROVIDES = [
    _ZigModuleInfo,
    DefaultInfo,
    InstrumentedFilesInfo,
    QualitySourcesInfo,
]

_DX_ZIG_EXEC_PROVIDES = [
    DefaultInfo,
    QualitySourcesInfo,
]

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
)

def zig_quality_srcs(main, srcs):
    """Returns the hand-written sources a Zig target owns: main first, then srcs."""
    owned = [main] if main != None else []
    return owned + [s for s in (srcs or []) if s != main]

def zig_import_name(name, import_name):
    """Returns the module import name, defaulting to the public target name."""
    return import_name if import_name else name

def _upstream_kwargs(main, srcs, kwargs):
    out = dict(kwargs)
    if main != None:
        out["main"] = main
    if srcs:
        out["srcs"] = srcs
    return out

def zig_library(name, main, srcs = None, import_name = None, visibility = None, tags = None, **kwargs):
    """Minimal wrapper over zig_library; @import uses the public name."""
    _zig_library(
        name = name + "_upstream",
        import_name = zig_import_name(name, import_name),
        tags = dx_upstream_tags(tags),
        visibility = ["//visibility:private"],
        **_upstream_kwargs(main, srcs, kwargs)
    )
    forward_kwargs = {}
    for key in ("aspect_hints", "testonly", "target_compatible_with"):
        if kwargs.get(key, None) != None:
            forward_kwargs[key] = kwargs[key]
    if tags != None:
        forward_kwargs["tags"] = tags
    _zig_library_forward(
        name = name,
        upstream = name + "_upstream",
        srcs = zig_quality_srcs(main, srcs),
        visibility = visibility,
        **forward_kwargs
    )

def zig_binary(name, main = None, srcs = None, visibility = None, **kwargs):
    """Minimal wrapper over zig_binary."""
    upstream = dx_binary_upstream_kwargs(_upstream_kwargs(main, srcs, kwargs), srcs or [])
    _zig_binary(
        name = name + "_upstream",
        **upstream
    )
    _zig_binary_forward(
        name = name,
        upstream = name + "_upstream",
        srcs = zig_quality_srcs(main, srcs),
        visibility = visibility,
        **dx_binary_forward_kwargs(kwargs)
    )

def zig_test(name, main = None, srcs = None, visibility = None, **kwargs):
    """Minimal wrapper over zig_test."""
    _zig_test(
        name = name + "_upstream",
        **dx_test_upstream_kwargs(_upstream_kwargs(main, srcs, kwargs))
    )
    _zig_forward_test(
        name = name,
        testonly = True,
        upstream = name + "_upstream",
        srcs = zig_quality_srcs(main, srcs),
        visibility = visibility,
        **dx_test_forward_kwargs(kwargs)
    )
