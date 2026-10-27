"""Minimal Java wrappers."""

load("@rules_java//java:defs.bzl", _java_binary = "java_binary", _java_library = "java_library", _java_test = "java_test")
load("@rules_java//java/common:java_info.bzl", "JavaInfo")
load("@rules_java//java/common/rules:java_helper.bzl", "helper")
load("//libs/starlark:wrapper.bzl", "DX_EXEC_PROVIDES", "dx_append_flags", "dx_executable_forward_rule", "dx_lcov_merger_attr", "dx_library_forward_rule", "dx_src_path", "dx_wrap", "dx_wrap_binary", "dx_wrap_test")
load("//quality:sources.bzl", "QualitySourcesInfo")

_DX_JAVA_LIBRARY_PROVIDES = [
    JavaInfo,
    DefaultInfo,
    InstrumentedFilesInfo,
    QualitySourcesInfo,
]

_DX_JAVA_SOURCE_SPECS = [("java", "java")]

_java_library_forward = dx_library_forward_rule(
    provides = _DX_JAVA_LIBRARY_PROVIDES,
    required_providers = [(JavaInfo, "JavaInfo")],
    quality_specs = _DX_JAVA_SOURCE_SPECS,
    what = "java_*",
    allow_files = True,
    upstream_providers = [[JavaInfo]],
)

_java_binary_forward = dx_executable_forward_rule(
    kind = "executable",
    provides = DX_EXEC_PROVIDES,
    required_providers = [],
    quality_specs = _DX_JAVA_SOURCE_SPECS,
    what = "java_*",
    allow_files = True,
    upstream_providers = [[JavaInfo]],
    optional_providers = [JavaInfo],
    runtime = "besteffort",
)

_java_forward_test = dx_executable_forward_rule(
    kind = "test",
    provides = DX_EXEC_PROVIDES,
    required_providers = [],
    quality_specs = _DX_JAVA_SOURCE_SPECS,
    what = "java_*",
    allow_files = True,
    upstream_providers = [[JavaInfo]],
    extra_attrs = dx_lcov_merger_attr(),
    optional_providers = [JavaInfo],
)

def java_javacopts_with_werror(kwargs):
    """Returns kwargs with -Werror plus -Xlint:all enforced on javacopts."""
    return dx_append_flags(kwargs, "javacopts", ["-Werror", "-Xlint:all"])

def java_entry_class(name, srcs, package):
    """Returns the class java_* infers for the wrapper name, or None."""
    if type(srcs) != "list":
        return None
    path = None
    for src in srcs:
        src_path = dx_src_path(package, src)
        if src_path != None and src_path.split("/")[-1] == name + ".java":
            path = src_path[:-len(".java")]
            break
    if path == None:
        path = helper.get_relative(package, name)
    segments = helper.java_segments(path)
    return ".".join(segments) if segments != None else None

def _java_unset(value):
    """Returns whether an entry attribute counts as unset."""
    return value == None or value == ""

def java_entry_kwargs(name, srcs, package, kwargs, attr):
    """Returns kwargs with attr set to the inferred class when upstream would infer one."""
    if not _java_unset(kwargs.get(attr)):
        return kwargs
    use_testrunner = kwargs.get("use_testrunner")
    create_executable = kwargs.get("create_executable")
    if attr == "test_class":
        if use_testrunner != None and use_testrunner != True:
            return kwargs
    else:
        if create_executable != None and create_executable != True:
            return kwargs
        if use_testrunner != None and use_testrunner != False:
            return kwargs
        if type(srcs) != "list" or not srcs:
            return kwargs
    entry = java_entry_class(name, srcs, package)
    if entry == None:
        return kwargs
    return dict(kwargs, **{attr: entry})

def java_library(name, srcs = None, visibility = None, **kwargs):
    """Minimal wrapper over java_library."""
    dx_wrap(name, _java_library, _java_library_forward, srcs, visibility = visibility, upstream_kwargs = java_javacopts_with_werror(kwargs), **kwargs)

def java_binary(name, srcs = None, visibility = None, **kwargs):
    """Minimal wrapper over java_binary."""
    upstream_kwargs = java_entry_kwargs(name, srcs, native.package_name(), java_javacopts_with_werror(kwargs), "main_class")
    dx_wrap_binary(name, _java_binary, _java_binary_forward, srcs, visibility = visibility, upstream_kwargs = upstream_kwargs, **kwargs)

def java_test(name, srcs = None, visibility = None, **kwargs):
    """Minimal wrapper over java_test."""
    upstream_kwargs = java_entry_kwargs(name, srcs, native.package_name(), java_javacopts_with_werror(kwargs), "test_class")
    dx_wrap_test(name, _java_test, _java_forward_test, srcs, visibility = visibility, upstream_kwargs = upstream_kwargs, **kwargs)
