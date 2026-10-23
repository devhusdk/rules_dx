"""Minimal Dart wrappers."""

load("@rules_dart//dart:defs.bzl", _dart_binary = "dart_binary", _dart_codegen = "dart_codegen", _dart_library = "dart_library", _dart_package_metadata = "dart_package_metadata", _dart_test = "dart_test")
load("@rules_dart//dart:providers.bzl", "DartAnalyzableInfo", "DartCompileInfo", "DartInfo")
load("//libs/starlark:wrapper.bzl", "dx_binary_forward_kwargs", "dx_executable_forward_rule", "dx_lcov_merger_attr", "dx_library_forward_rule", "dx_test_forward_kwargs", "dx_test_upstream_kwargs", "dx_wrap")
load("//quality:sources.bzl", "QualitySourcesInfo")

DART_SOURCE_SPECS = [("dart", "dart")]

_DART_SOURCE_EXTS = [".dart"]

CROSS_COMPILE_ERROR = "dart cross-compilation to %s-%s is not supported: dart compile downloads the target snapshotter at action time"

def dart_cross_target_error(target_os, target_arch):
    """Returns the refusal for a cross-compiling Dart SDK, or an empty string."""
    if target_os == "" and target_arch == "":
        return ""
    return CROSS_COMPILE_ERROR % (target_os or "host", target_arch or "host")

def _dart_native_target_impl(ctx):
    """Fails analysis when the Dart SDK resolved for the target cross-compiles."""
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
    provides = [DartInfo, DefaultInfo, InstrumentedFilesInfo, QualitySourcesInfo],
    required_providers = [(DartInfo, "DartInfo")],
    quality_specs = DART_SOURCE_SPECS,
    what = "dart_library",
    allow_files = _DART_SOURCE_EXTS,
    upstream_providers = [[DartInfo]],
    runtime = "besteffort",
)

_dart_binary_forward = dx_executable_forward_rule(
    kind = "executable",
    provides = [DartAnalyzableInfo, DartCompileInfo, DefaultInfo, QualitySourcesInfo],
    required_providers = [(DartAnalyzableInfo, "DartAnalyzableInfo"), (DartCompileInfo, "DartCompileInfo")],
    quality_specs = DART_SOURCE_SPECS,
    what = "dart_binary",
    allow_files = _DART_SOURCE_EXTS,
    upstream_providers = [[DartAnalyzableInfo, DartCompileInfo]],
    extra_attrs = _NATIVE_TARGET_ATTR,
    runtime = "besteffort",
)

_dart_forward_test = dx_executable_forward_rule(
    kind = "test",
    provides = [DartAnalyzableInfo, DefaultInfo, QualitySourcesInfo],
    required_providers = [(DartAnalyzableInfo, "DartAnalyzableInfo")],
    quality_specs = DART_SOURCE_SPECS,
    what = "dart_test",
    allow_files = _DART_SOURCE_EXTS,
    upstream_providers = [[DartAnalyzableInfo]],
    extra_attrs = dict(dx_lcov_merger_attr(), **_NATIVE_TARGET_ATTR),
    runtime = "besteffort",
)

dart_package_metadata = _dart_package_metadata

dart_codegen = _dart_codegen

def dart_entry_sources(main, srcs):
    """Returns the sources a Dart executable owns: its entrypoint, then its srcs."""
    return [main] + [src for src in (srcs or []) if src != main]

def dart_root_package_name(package, name, kwargs):
    """Returns the package name a root-package library needs, or None."""
    if package != "" or "package_name" in kwargs or "package" in kwargs:
        return None
    return name

def dart_library(name, srcs = [], visibility = None, **kwargs):
    """Minimal wrapper over dart_library."""
    root_name = dart_root_package_name(native.package_name(), name, kwargs)
    if root_name != None:
        kwargs["package_name"] = root_name
    dx_wrap(name, _dart_library, _dart_library_forward, srcs, visibility = visibility, **kwargs)

def dart_binary(name, main, srcs = None, visibility = None, **kwargs):
    """Minimal wrapper over dart_binary."""
    upstream = dict(kwargs)
    if srcs:
        upstream["srcs"] = srcs
    upstream["visibility"] = ["//visibility:private"]
    _dart_binary(
        name = name + "_upstream",
        main = main,
        **upstream
    )
    _dart_binary_forward(
        name = name,
        upstream = name + "_upstream",
        srcs = dart_entry_sources(main, srcs),
        visibility = visibility,
        **dx_binary_forward_kwargs(kwargs)
    )

def dart_test(name, main, srcs = None, visibility = None, **kwargs):
    """Minimal wrapper over dart_test."""
    upstream = dx_test_upstream_kwargs(kwargs, srcs = srcs)
    _dart_test(
        name = name + "_upstream",
        main = main,
        **upstream
    )
    _dart_forward_test(
        name = name,
        testonly = True,
        upstream = name + "_upstream",
        srcs = dart_entry_sources(main, srcs),
        visibility = visibility,
        **dx_test_forward_kwargs(kwargs)
    )
