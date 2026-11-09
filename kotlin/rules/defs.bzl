"""Minimal Kotlin wrappers."""

load("@rules_java//java:defs.bzl", "JavaInfo")
load("@rules_kotlin//kotlin:core.bzl", "kt_javac_options")
load("@rules_kotlin//kotlin:jvm.bzl", _kt_jvm_binary = "kt_jvm_binary", _kt_jvm_library = "kt_jvm_library", _kt_jvm_test = "kt_jvm_test")
load("@rules_kotlin//kotlin/internal:opts.bzl", "KotlincOptions")
load("//java/rules:coverage.bzl", "DX_JVM_TEST_RUNNER", "dx_jvm_test_coverage")
load("//java/rules:defs.bzl", "java_jmh_test")
load("//java/rules:jmh.bzl", "jmh_bench")
load("//libs/starlark:wrapper.bzl", "DX_EXEC_PROVIDES", "dx_executable_forward_rule", "dx_lcov_merger_attr", "dx_library_forward_rule", "dx_src_path", "dx_warnings_as_errors", "dx_without_warnings_attr", "dx_wrap", "dx_wrap_binary", "dx_wrap_test")
load("//quality:sources.bzl", "QualitySourcesInfo")

_KT_TOOLCHAIN_TYPE = "@rules_kotlin//kotlin/internal:kt_toolchain_type"

_KOTLIN_TEST_ROOTS = [
    "src/test/java/",
    "src/test/kotlin/",
    "javatests/",
    "kotlin/",
    "java/",
    "test/",
]

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

_DX_KOTLIN_SOURCE_SPECS = [("kotlin", ["kt", "kts"]), ("java", "java")]

_kotlin_library_forward = dx_library_forward_rule(
    provides = _DX_KOTLIN_LIBRARY_PROVIDES,
    required_providers = [(JavaInfo, "JavaInfo")],
    quality_specs = _DX_KOTLIN_SOURCE_SPECS,
    what = "kotlin_*",
    allow_files = True,
    upstream_providers = [[JavaInfo]],
)

_kotlin_binary_forward = dx_executable_forward_rule(
    kind = "executable",
    provides = DX_EXEC_PROVIDES,
    required_providers = [],
    quality_specs = _DX_KOTLIN_SOURCE_SPECS,
    what = "kotlin_*",
    allow_files = True,
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
    allow_files = True,
    upstream_providers = [[JavaInfo]],
    extra_attrs = dict(
        dx_lcov_merger_attr(),
        main_class = attr.string(default = DX_JVM_TEST_RUNNER),
    ),
    optional_providers = [JavaInfo],
    coverage_providers = dx_jvm_test_coverage,
)

def _kotlin_warnings_as_errors_opts_impl(ctx):
    info = ctx.attr.opts[KotlincOptions]
    fields = {}
    for field in dir(info):
        if field.startswith("to_"):
            continue
        fields[field] = getattr(info, field)
    if ctx.attr.warnings_as_errors:
        fields["warn"] = "error"
    return [KotlincOptions(**fields)]

kotlin_warnings_as_errors_opts = rule(
    implementation = _kotlin_warnings_as_errors_opts_impl,
    doc = "Copies kotlinc options and turns warnings into errors when warnings_as_errors is set.",
    attrs = {
        "opts": attr.label(default = "//kotlin/rules:base_opts", providers = [KotlincOptions], doc = "Kotlinc options to copy."),
        "warnings_as_errors": attr.bool(doc = "Whether warnings are errors."),
    },
    provides = [KotlincOptions],
)

def _kotlin_has_own_opts(kwargs):
    return kwargs.get("kotlinc_opts") != None or kwargs.get("warnings_as_errors") != None

def kotlin_kotlinc_opts_with_warnings_as_errors(name, kwargs):
    """Returns kwargs with kotlinc_opts routed through the shared default or a per-target options rule."""
    out = dx_without_warnings_attr(kwargs)
    if _kotlin_has_own_opts(kwargs):
        out["kotlinc_opts"] = ":" + name + "_kotlinc_opts"
    else:
        out["kotlinc_opts"] = str(Label("//kotlin/rules:warnings_as_errors"))
    return out

def _kotlin_declare_warnings_as_errors_opts(name, kwargs):
    """Declares the per-target options rule when kotlinc_opts or warnings_as_errors is set."""
    if not _kotlin_has_own_opts(kwargs):
        return
    extra = {key: kwargs[key] for key in ("tags", "testonly", "target_compatible_with") if kwargs.get(key) != None}
    if kwargs.get("kotlinc_opts") != None:
        extra["opts"] = kwargs["kotlinc_opts"]
    _, warning_flags = dx_warnings_as_errors(kwargs, True, False)
    kotlin_warnings_as_errors_opts(name = name + "_kotlinc_opts", warnings_as_errors = warning_flags, visibility = ["//visibility:private"], **extra)

def kotlin_test_class(name, srcs, package):
    """Returns the test class kt_jvm_test infers for the wrapper name, or None."""
    if type(srcs) != "list":
        return None
    for src in srcs:
        path = dx_src_path(package, src)
        if path == None:
            continue
        relative = path.replace(package + "/", "") if package else path
        if relative.split(".")[0] != name and path.split("/")[-1].split(".")[0] != name:
            continue
        for root in _KOTLIN_TEST_ROOTS:
            parts = path.split(root, 1)
            if len(parts) == 2:
                return parts[1].split(".")[0].replace("/", ".")
    return None

def kotlin_test_entry_kwargs(name, srcs, package, kwargs):
    """Returns kwargs with test_class set to the inferred class when unset."""
    if kwargs.get("test_class") not in (None, ""):
        return kwargs
    test_class = kotlin_test_class(name, srcs, package)
    if test_class == None:
        return kwargs
    return dict(kwargs, test_class = test_class)

def kotlin_library(name, srcs = None, visibility = None, **kwargs):
    """Minimal wrapper over kt_jvm_library."""
    _kotlin_declare_warnings_as_errors_opts(name, kwargs)
    dx_wrap(name, _kt_jvm_library, _kotlin_library_forward, srcs, visibility = visibility, upstream_kwargs = kotlin_kotlinc_opts_with_warnings_as_errors(name, kwargs), **dx_without_warnings_attr(kwargs))

def kotlin_binary(name, srcs = None, visibility = None, **kwargs):
    """Minimal wrapper over kt_jvm_binary."""
    _kotlin_declare_warnings_as_errors_opts(name, kwargs)
    dx_wrap_binary(name, _kt_jvm_binary, _kotlin_binary_forward, srcs, visibility = visibility, upstream_kwargs = kotlin_kotlinc_opts_with_warnings_as_errors(name, kwargs), **dx_without_warnings_attr(kwargs))

def kotlin_test(name, srcs = None, visibility = None, **kwargs):
    """Minimal wrapper over kt_jvm_test."""
    _kotlin_declare_warnings_as_errors_opts(name, kwargs)
    package = native.package_name()
    upstream_kwargs = kotlin_test_entry_kwargs(name, srcs, package, kotlin_kotlinc_opts_with_warnings_as_errors(name, kwargs))
    if upstream_kwargs.get("test_class") in (None, "") and upstream_kwargs.get("main_class") in (None, ""):
        fail("kotlin_test //{}:{}: cannot infer test_class from srcs; set test_class.".format(package, name))
    dx_wrap_test(
        name,
        _kt_jvm_test,
        _kotlin_forward_test,
        srcs,
        visibility = visibility,
        upstream_kwargs = upstream_kwargs,
        extra_forward_kwargs = {"main_class": upstream_kwargs.get("main_class") or DX_JVM_TEST_RUNNER},
        **dx_without_warnings_attr(kwargs)
    )

def kotlin_bench(name, srcs, deps = [], tags = [], **kwargs):
    """Runs the JMH benchmarks in Kotlin srcs and writes dx-bench.json for dx bench.

    Args:
      name: the test target.
      srcs: Kotlin sources with @Benchmark methods.
      deps: libraries the benchmarks use.
      tags: extra tags. dx-bench and manual are added.
      **kwargs: passed to the test.
    """
    kt_javac_options(
        name = name + "_javac_opts",
        warn = "off",
        testonly = True,
        visibility = ["//visibility:private"],
    )
    jmh_bench(name, kotlin_library, java_jmh_test, srcs, deps, tags, kwargs, {"javac_opts": ":" + name + "_javac_opts"})
