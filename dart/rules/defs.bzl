"""Thin Dart wrappers over rules_dart."""

load("@rules_dart//dart:defs.bzl", _dart_binary = "dart_binary", _dart_library = "dart_library", _dart_package_metadata = "dart_package_metadata", _dart_test = "dart_test")
load("@rules_dart//dart:providers.bzl", "DartAnalyzableInfo", "DartCompileInfo", "DartInfo")
load("//libs/starlark:wrapper.bzl", "DX_EXEC_PROVIDES", "dx_executable_forward_rule", "dx_lcov_merger_attr", "dx_library_forward_rule", "dx_wrap", "dx_wrap_binary", "dx_wrap_test")
load("//quality:sources.bzl", "QualitySourcesInfo", "QualityToolsInfo")

DART_SOURCE_SPECS = [("dart", "dart")]

DART_SOURCE_EXTS = [".dart"]

DART_COMPATIBLE_WITH = select({
    Label("//dart/rules:rules_dart_platform"): [],
    "//conditions:default": [Label("@platforms//:incompatible")],
})

DART_DEFERRED_SKIP_TAGS = ["no-lint", "no-typecheck"]

DART_QUALITY_TOOLS = {"dart": Label("//dart/tools:dart")}

CROSS_COMPILE_ERROR = "dart cross-compilation to %s-%s is not supported: dart compile downloads the target snapshotter at action time"

def dart_cross_target_error(target_os, target_arch):
    """Returns the refusal for a cross-compiling Dart SDK, or an empty string."""
    if target_os == "" and target_arch == "":
        return ""
    return CROSS_COMPILE_ERROR % (target_os or "host", target_arch or "host")

def _dart_native_target_impl(ctx):
    sdk = ctx.toolchains["@rules_dart//dart:toolchain_type"].dart_sdk_info
    err = dart_cross_target_error(sdk.target_os, sdk.target_arch)
    if err != "":
        fail(err)
    return [DefaultInfo()]

dart_native_target = rule(
    implementation = _dart_native_target_impl,
    doc = "Requires a native Dart SDK for the target platform.",
    toolchains = ["@rules_dart//dart:toolchain_type"],
)

_NATIVE_TARGET_ATTR = {
    "_native_target": attr.label(default = Label("//dart/rules:native_target")),
}

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
    extra_attrs = _NATIVE_TARGET_ATTR,
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
    extra_attrs = dict(dx_lcov_merger_attr(), **_NATIVE_TARGET_ATTR),
    runtime = "besteffort",
)

dart_package_metadata = _dart_package_metadata

def dart_entry_sources(main, srcs):
    """Returns the sources a Dart executable owns: its entrypoint, then its srcs."""
    return [main] + [src for src in (srcs or []) if src != main]

def dart_root_package_name(package, name, kwargs):
    """Returns the package name a root-package library needs, or None."""
    if package != "" or "package_name" in kwargs or "package" in kwargs:
        return None
    return name

def dart_compatible_with(target_compatible_with):
    """Appends the rules_dart platform guard to the caller's constraints."""
    return (target_compatible_with or []) + DART_COMPATIBLE_WITH

def dart_deferred_tags(tags):
    """Adds the skip tags that keep the deferred dart class out of quality aspects."""
    out = list(tags or [])
    for tag in DART_DEFERRED_SKIP_TAGS:
        if tag not in out:
            out.append(tag)
    return out

def _dart_kwargs(kwargs):
    return dict(
        kwargs,
        tags = dart_deferred_tags(kwargs.get("tags")),
        target_compatible_with = dart_compatible_with(kwargs.get("target_compatible_with")),
    )

def dart_library(name, srcs = None, visibility = None, **kwargs):
    """Thin wrapper over dart_library."""
    kwargs = _dart_kwargs(kwargs)
    root_name = dart_root_package_name(native.package_name(), name, kwargs)
    if root_name != None:
        kwargs["package_name"] = root_name
    dx_wrap(name, _dart_library, _dart_library_forward, srcs or [], visibility = visibility, **kwargs)

def dart_binary(name, main, srcs = None, visibility = None, **kwargs):
    """Thin wrapper over dart_binary."""
    kwargs = _dart_kwargs(kwargs)
    dx_wrap_binary(name, _dart_binary, _dart_binary_forward, srcs, visibility = visibility, upstream_kwargs = dict(kwargs, main = main), quality_srcs = dart_entry_sources(main, srcs), **kwargs)

def dart_test(name, main, srcs = None, visibility = None, **kwargs):
    """Thin wrapper over dart_test."""
    kwargs = _dart_kwargs(kwargs)
    dx_wrap_test(name, _dart_test, _dart_forward_test, srcs, visibility = visibility, upstream_kwargs = dict(kwargs, main = main), quality_srcs = dart_entry_sources(main, srcs), **kwargs)
