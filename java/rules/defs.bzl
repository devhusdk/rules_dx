"""Minimal Java wrappers."""

load("@rules_java//java:defs.bzl", _java_binary = "java_binary", _java_library = "java_library", _java_test = "java_test")
load("@rules_java//java/common:java_info.bzl", "JavaInfo")
load("@rules_java//java/common/rules:java_helper.bzl", "helper")
load("@rules_shell//shell:sh_binary.bzl", "sh_binary")
load("//java/rules:coverage.bzl", "DX_JVM_TEST_RUNNER", "dx_jvm_test_coverage")
load("//libs/starlark:wrapper.bzl", "DX_EXEC_PROVIDES", "dx_append_flags", "dx_executable_forward_rule", "dx_lcov_merger_attr", "dx_library_forward_rule", "dx_src_path", "dx_warnings_as_errors", "dx_without_warnings_attr", "dx_wrap", "dx_wrap_binary", "dx_wrap_test")
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
    extra_attrs = dict(
        dx_lcov_merger_attr(),
        main_class = attr.string(default = DX_JVM_TEST_RUNNER),
    ),
    optional_providers = [JavaInfo],
    coverage_providers = dx_jvm_test_coverage,
)

def java_javacopts_with_warnings_as_errors(kwargs):
    """Returns kwargs with -Xlint:all and, unless turned off, -Werror on javacopts."""
    out, warning_flags = dx_warnings_as_errors(kwargs, ["-Werror"])
    out = dx_append_flags(out, "javacopts", ["-Xlint:all"])
    return dx_append_flags(out, "javacopts", warning_flags)

def java_entry_class(name, srcs, package):
    """Returns the class java_* infers for the wrapper name, or None without a Java root."""
    path = None
    if type(srcs) == "list":
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

def _java_entry_wanted(srcs, kwargs, attr):
    """Returns whether upstream would infer attr for these kwargs."""
    if not _java_unset(kwargs.get(attr)):
        return False
    use_testrunner = kwargs.get("use_testrunner")
    create_executable = kwargs.get("create_executable")
    if attr == "test_class":
        return use_testrunner == None or use_testrunner == True
    if create_executable != None and create_executable != True:
        return False
    if use_testrunner != None and use_testrunner != False:
        return False
    return srcs != None and srcs != []

def java_entry_kwargs(name, srcs, package, kwargs, attr):
    """Returns kwargs with attr set to the inferred class when upstream would infer one."""
    if not _java_entry_wanted(srcs, kwargs, attr):
        return kwargs
    entry = java_entry_class(name, srcs, package)
    if entry == None:
        return kwargs
    return dict(kwargs, **{attr: entry})

def java_entry_error(name, srcs, package, kwargs, attr):
    """Returns the error for a wrapper that needs attr but cannot infer it, or None."""
    if not _java_entry_wanted(srcs, kwargs, attr):
        return None
    if java_entry_class(name, srcs, package) != None:
        return None
    kind = "java_test" if attr == "test_class" else "java_binary"
    return "%s //%s:%s: cannot infer %s from srcs or package; set %s." % (kind, package, name, attr, attr)

def _java_entry_kwargs_or_fail(name, srcs, kwargs, attr):
    package = native.package_name()
    error = java_entry_error(name, srcs, package, kwargs, attr)
    if error:
        fail(error)
    return java_entry_kwargs(name, srcs, package, kwargs, attr)

def java_library(name, srcs = None, visibility = None, **kwargs):
    """Minimal wrapper over java_library."""
    upstream_kwargs = java_javacopts_with_warnings_as_errors(kwargs)
    dx_wrap(name, _java_library, _java_library_forward, srcs, visibility = visibility, upstream_kwargs = upstream_kwargs, **dx_without_warnings_attr(kwargs))

def java_binary(name, srcs = None, visibility = None, **kwargs):
    """Minimal wrapper over java_binary."""
    upstream_kwargs = _java_entry_kwargs_or_fail(name, srcs, java_javacopts_with_warnings_as_errors(kwargs), "main_class")
    dx_wrap_binary(name, _java_binary, _java_binary_forward, srcs, visibility = visibility, upstream_kwargs = upstream_kwargs, **dx_without_warnings_attr(kwargs))

def java_test(name, srcs = None, visibility = None, **kwargs):
    """Minimal wrapper over java_test."""
    upstream_kwargs = _java_entry_kwargs_or_fail(name, srcs, java_javacopts_with_warnings_as_errors(kwargs), "test_class")
    main_class = kwargs.get("main_class") if kwargs.get("use_testrunner") == False else DX_JVM_TEST_RUNNER
    dx_wrap_test(
        name,
        _java_test,
        _java_forward_test,
        srcs,
        visibility = visibility,
        upstream_kwargs = upstream_kwargs,
        extra_forward_kwargs = {"main_class": main_class},
        **dx_without_warnings_attr(kwargs)
    )

def java_fuzz_test(name, srcs = None, target_class = None, visibility = None, tags = None, deps = None, **kwargs):
    """A Jazzer fuzz target plus a `<name>_run` runner for `dx fuzz`."""
    target_class = target_class or java_entry_class(name, srcs, native.package_name())
    if target_class == None:
        fail("java_fuzz_test %s needs target_class" % name)
    java_library(
        name = name + "_lib",
        srcs = srcs,
        deps = deps,
        testonly = True,
        tags = ["manual"],
        visibility = ["//visibility:private"],
        **kwargs
    )
    _java_binary(
        name = name,
        main_class = "com.code_intelligence.jazzer.Jazzer",
        runtime_deps = [":" + name + "_lib", Label("@dx_jazzer//jar"), Label("@dx_jazzer_api//jar")],
        testonly = True,
        tags = (tags or []) + ["dx-fuzz", "manual"],
        visibility = visibility,
    )
    sh_binary(
        name = name + "_run",
        srcs = [Label("//java/rules:fuzz_run.sh")],
        args = ["$(rootpath :" + name + ")", target_class],
        data = [":" + name],
        testonly = True,
        visibility = visibility,
    )
