"""Minimal Kotlin wrappers."""

load("@rules_java//java:defs.bzl", "JavaInfo")
load("@rules_kotlin//kotlin:jvm.bzl", _kt_jvm_binary = "kt_jvm_binary", _kt_jvm_library = "kt_jvm_library", _kt_jvm_test = "kt_jvm_test")
load("@rules_kotlin//kotlin/internal:opts.bzl", "KotlincOptions")
load("//libs/starlark:wrapper.bzl", "DX_EXEC_PROVIDES", "dx_executable_forward_rule", "dx_lcov_merger_attr", "dx_library_forward_rule", "dx_wrap", "dx_wrap_binary", "dx_wrap_test")
load("//quality:sources.bzl", "QualitySourcesInfo")

_KT_TOOLCHAIN_TYPE = "@rules_kotlin//kotlin/internal:kt_toolchain_type"

def _kotlin_coverage_runfiles(ctx):
    """Returns the Jacoco runner runfiles the upstream launcher needs under coverage."""
    if not ctx.configuration.coverage_enabled:
        return None
    runner = ctx.toolchains[_KT_TOOLCHAIN_TYPE].jacocorunner
    if runner == None:
        return None
    return ctx.runfiles(transitive_files = depset(runner.files.to_list()))

_DX_KOTLIN_LIBRARY_PROVIDES = [
    JavaInfo,
    DefaultInfo,
    InstrumentedFilesInfo,
    QualitySourcesInfo,
]

_DX_KOTLIN_SOURCE_SPECS = [("kotlin", ["kt", "kts"])]
_DX_KOTLIN_SOURCE_EXTS = [".kt", ".kts"]

_kotlin_library_forward = dx_library_forward_rule(
    provides = _DX_KOTLIN_LIBRARY_PROVIDES,
    required_providers = [(JavaInfo, "JavaInfo")],
    quality_specs = _DX_KOTLIN_SOURCE_SPECS,
    what = "kotlin_*",
    allow_files = _DX_KOTLIN_SOURCE_EXTS,
    upstream_providers = [[JavaInfo]],
)

_kotlin_binary_forward = dx_executable_forward_rule(
    kind = "executable",
    provides = DX_EXEC_PROVIDES,
    required_providers = [],
    quality_specs = _DX_KOTLIN_SOURCE_SPECS,
    what = "kotlin_*",
    allow_files = _DX_KOTLIN_SOURCE_EXTS,
    upstream_providers = [[JavaInfo]],
    optional_providers = [JavaInfo],
    runtime = "besteffort",
    coverage_runfiles = _kotlin_coverage_runfiles,
    toolchains = [_KT_TOOLCHAIN_TYPE],
)

_kotlin_forward_test = dx_executable_forward_rule(
    kind = "test",
    provides = DX_EXEC_PROVIDES,
    required_providers = [],
    quality_specs = _DX_KOTLIN_SOURCE_SPECS,
    what = "kotlin_*",
    allow_files = _DX_KOTLIN_SOURCE_EXTS,
    upstream_providers = [[JavaInfo]],
    extra_attrs = dx_lcov_merger_attr(),
    optional_providers = [JavaInfo],
)

def _kotlin_werror_opts_impl(ctx):
    info = ctx.attr.opts[KotlincOptions]
    fields = {}
    for field in dir(info):
        if field.startswith("to_"):
            continue
        fields[field] = getattr(info, field)
    fields["warn"] = "error"
    return [KotlincOptions(**fields)]

_kotlin_werror_opts = rule(
    implementation = _kotlin_werror_opts_impl,
    doc = "Copies kotlinc options and forces warnings to errors.",
    attrs = {"opts": attr.label(mandatory = True, providers = [KotlincOptions], doc = "User kotlinc options.")},
    provides = [KotlincOptions],
)

def kotlin_kotlinc_opts_with_werror(name, kwargs):
    """Returns kwargs with kotlinc_opts routed through the warnings-as-errors default or wrapper."""
    if kwargs.get("kotlinc_opts") == None:
        out = dict(kwargs)
        out["kotlinc_opts"] = str(Label("//kotlin/rules:warnings_as_errors"))
        return out
    return dict(kwargs, kotlinc_opts = ":" + name + "_kotlinc_opts")

def _kotlin_declare_werror_opts(name, kwargs):
    """Declares the wrapper target when the user set kotlinc_opts."""
    if kwargs.get("kotlinc_opts") == None:
        return
    extra = {key: kwargs[key] for key in ("tags", "testonly", "target_compatible_with") if kwargs.get(key) != None}
    _kotlin_werror_opts(name = name + "_kotlinc_opts", opts = kwargs["kotlinc_opts"], visibility = ["//visibility:private"], **extra)

def kotlin_library(name, srcs, visibility = None, **kwargs):
    """Minimal wrapper over kt_jvm_library."""
    _kotlin_declare_werror_opts(name, kwargs)
    dx_wrap(name, _kt_jvm_library, _kotlin_library_forward, srcs, visibility = visibility, upstream_kwargs = kotlin_kotlinc_opts_with_werror(name, kwargs), **kwargs)

def kotlin_binary(name, srcs = None, visibility = None, **kwargs):
    """Minimal wrapper over kt_jvm_binary."""
    _kotlin_declare_werror_opts(name, kwargs)
    dx_wrap_binary(name, _kt_jvm_binary, _kotlin_binary_forward, srcs, visibility = visibility, upstream_kwargs = kotlin_kotlinc_opts_with_werror(name, kwargs), **kwargs)

def kotlin_test(name, srcs, visibility = None, **kwargs):
    """Minimal wrapper over kt_jvm_test."""
    _kotlin_declare_werror_opts(name, kwargs)
    dx_wrap_test(name, _kt_jvm_test, _kotlin_forward_test, srcs, visibility = visibility, upstream_kwargs = kotlin_kotlinc_opts_with_werror(name, kwargs), **kwargs)
