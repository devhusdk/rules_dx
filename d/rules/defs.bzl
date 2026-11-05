"""Thin D wrappers over rules_d."""

load("@rules_d//d:defs.bzl", _d_binary = "d_binary", _d_library = "d_library", _d_test = "d_test")
load("@rules_d//d/private:providers.bzl", _DInfo = "DInfo")
load("//d/toolchain:pins.bzl", "LDC_CELLS")
load("//libs/starlark:cells.bzl", "cells_compatible_with")
load("//libs/starlark:wrapper.bzl", "DX_EXEC_PROVIDES", "dx_append_flags", "dx_executable_forward_rule", "dx_language_kwargs", "dx_lcov_merger_attr", "dx_library_forward_rule", "dx_upstream_tags", "dx_wrap", "dx_wrap_binary", "dx_wrap_test")
load("//quality:sources.bzl", "QualitySourcesInfo", "QualityToolsInfo")

DInfo = _DInfo

D_CELLS = sorted(LDC_CELLS.keys())

D_COMPATIBLE_WITH = cells_compatible_with(LDC_CELLS, "//d/toolchain", D_CELLS, unsupported = True)

D_QUALITY_TOOLS = {
    "dfmt": Label("//d/tools:dfmt"),
    "dscanner": Label("//d/tools:dscanner"),
}

_D_EXEC_PROVIDES = DX_EXEC_PROVIDES + [QualityToolsInfo]

_SPECS = [("d", "d")]

_EXTS = [".d", ".di"]

_d_library_forward = dx_library_forward_rule(
    provides = [_DInfo, DefaultInfo, InstrumentedFilesInfo, QualitySourcesInfo, QualityToolsInfo],
    required_providers = [(_DInfo, "DInfo")],
    quality_specs = _SPECS,
    what = "d_library",
    allow_files = _EXTS,
    upstream_providers = [[_DInfo]],
    runtime = "besteffort",
    instrument_srcs = True,
    quality_tools = D_QUALITY_TOOLS,
)

_d_binary_forward = dx_executable_forward_rule(
    kind = "executable",
    provides = _D_EXEC_PROVIDES,
    required_providers = [],
    quality_specs = _SPECS,
    what = "d_binary",
    allow_files = _EXTS,
    upstream_providers = None,
    runtime = "besteffort",
    quality_tools = D_QUALITY_TOOLS,
)

def _d_test_coverage(ctx, upstream):
    """Runs the upstream test under the launcher that turns ldc's .lst files into LCOV."""
    instrumented = coverage_common.instrumented_files_info(
        ctx,
        source_attributes = ["srcs"],
        dependency_attributes = ["upstream", "deps"],
    )
    sources = depset(ctx.files.srcs, transitive = [dep[DInfo].interface_srcs for dep in ctx.attr.deps if DInfo in dep])
    exe = upstream[DefaultInfo].files_to_run.executable
    launcher = ctx.attr._d_coverage_launcher[DefaultInfo]
    env = {}
    inherited = []
    if RunEnvironmentInfo in upstream:
        env.update(upstream[RunEnvironmentInfo].environment)
        inherited = upstream[RunEnvironmentInfo].inherited_environment
    short = exe.short_path
    env["DX_D_COVERAGE_UPSTREAM"] = short[3:] if short.startswith("../") else ctx.workspace_name + "/" + short
    return struct(
        instrumented = instrumented,
        env = RunEnvironmentInfo(environment = env, inherited_environment = inherited),
        runfiles = ctx.runfiles(files = [exe], transitive_files = sources).merge(launcher.default_runfiles),
        executable = ctx.executable._d_coverage_launcher,
    )

_d_forward_test = dx_executable_forward_rule(
    kind = "test",
    provides = _D_EXEC_PROVIDES,
    required_providers = [],
    quality_specs = _SPECS,
    what = "d_test",
    allow_files = _EXTS,
    upstream_providers = None,
    extra_attrs = dx_lcov_merger_attr() | {
        "deps": attr.label_list(doc = "Dependencies of the wrapped test, whose sources count for coverage."),
        "_d_coverage_launcher": attr.label(
            default = Label("//tools/coverage/d:d_coverage_launcher"),
            executable = True,
            cfg = "target",
        ),
    },
    runtime = "besteffort",
    coverage_providers = _d_test_coverage,
    quality_tools = D_QUALITY_TOOLS,
)

def d_quality_srcs(srcs, string_srcs = None):
    """Returns the hand-written D sources a target owns: srcs without any string import file."""
    excluded = string_srcs or []
    return [s for s in (srcs or []) if s not in excluded]

def _string_srcs(name, kwargs):
    """Wraps string_srcs files in a filegroup, because rules_d takes only targets there."""
    files = kwargs.get("string_srcs")
    if not files:
        return kwargs
    native.filegroup(name = name + "_string_srcs", srcs = files, tags = ["manual"])
    return dict(kwargs, string_srcs = [":" + name + "_string_srcs"])

def _cover(kwargs):
    """Adds ldc's -cov to dopts under bazel coverage."""
    return dx_append_flags(kwargs, "dopts", select({
        Label("//d/rules:coverage"): ["-cov"],
        "//conditions:default": [],
    }))

def d_kwargs(kwargs):
    """Returns kwargs with the D platform guard appended."""
    return dx_language_kwargs(kwargs, [], D_COMPATIBLE_WITH)

def d_library(name, srcs, visibility = None, **kwargs):
    """Thin wrapper over the rules_d d_library rule."""
    kwargs = d_kwargs(kwargs)
    kwargs = _cover(kwargs)
    quality_srcs = d_quality_srcs(srcs, kwargs.get("string_srcs"))
    kwargs = _string_srcs(name, kwargs)
    dx_wrap(name, _d_library, _d_library_forward, srcs, visibility = visibility, quality_srcs = quality_srcs, **kwargs)

def d_binary(name, srcs, visibility = None, **kwargs):
    """Thin wrapper over the rules_d d_binary rule."""
    kwargs = d_kwargs(kwargs)
    quality_srcs = d_quality_srcs(srcs, kwargs.get("string_srcs"))
    kwargs = _string_srcs(name, kwargs)
    upstream = dict(kwargs, tags = dx_upstream_tags(kwargs.get("tags", None)))
    dx_wrap_binary(name, _d_binary, _d_binary_forward, srcs, visibility = visibility, upstream_kwargs = upstream, quality_srcs = quality_srcs, **kwargs)

D_COVERAGE_COLLECTOR = True

def d_test(name, srcs, visibility = None, **kwargs):
    """Thin wrapper over the rules_d d_test rule."""
    kwargs = d_kwargs(kwargs)
    quality_srcs = d_quality_srcs(srcs, kwargs.get("string_srcs"))
    kwargs = _cover(_string_srcs(name, kwargs))
    dx_wrap_test(name, _d_test, _d_forward_test, srcs, visibility = visibility, quality_srcs = quality_srcs, extra_forward_kwargs = {"deps": kwargs.get("deps", [])}, coverage_collector = D_COVERAGE_COLLECTOR, **kwargs)
