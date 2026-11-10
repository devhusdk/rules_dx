"""Thin D wrappers over rules_d."""

load("@bazel_skylib//rules:native_binary.bzl", "native_test")
load("@rules_d//d:defs.bzl", _d_binary = "d_binary", _d_library = "d_library", _d_test = "d_test")
load("@rules_d//d/private:providers.bzl", _DInfo = "DInfo")
load("//core:fuzz.bzl", "dx_fuzz_test")
load("//core:fuzz_bench.bzl", "dx_bench_test")
load("//core:wrapper.bzl", "DX_EXEC_PROVIDES", "dx_append_flags", "dx_executable_forward_rule", "dx_lcov_merger_attr", "dx_library_forward_rule", "dx_upstream_tags", "dx_wrap", "dx_wrap_binary", "dx_wrap_test")
load("//quality:sources.bzl", "QualitySourcesInfo")
load(":kwargs.bzl", "d_kwargs", _D_CELLS = "D_CELLS", _D_COMPATIBLE_WITH = "D_COMPATIBLE_WITH")

D_CELLS = _D_CELLS
D_COMPATIBLE_WITH = _D_COMPATIBLE_WITH

DInfo = _DInfo

_D_EXEC_PROVIDES = DX_EXEC_PROVIDES

_SPECS = [("d", "d")]

_EXTS = [".d", ".di"]

_d_library_forward = dx_library_forward_rule(
    provides = [_DInfo, DefaultInfo, InstrumentedFilesInfo, QualitySourcesInfo],
    required_providers = [(_DInfo, "DInfo")],
    quality_specs = _SPECS,
    what = "d_library",
    allow_files = _EXTS,
    upstream_providers = [[_DInfo]],
    runtime = "besteffort",
    instrument_srcs = True,
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
        Label("//d/private:coverage"): ["-cov"],
        "//conditions:default": [],
    }))

def d_library(name, srcs = None, visibility = None, **kwargs):
    """Thin wrapper over the rules_d d_library rule."""
    kwargs = d_kwargs(kwargs)
    kwargs = _cover(kwargs)
    quality_srcs = d_quality_srcs(srcs, kwargs.get("string_srcs"))
    kwargs = _string_srcs(name, kwargs)
    dx_wrap(name, _d_library, _d_library_forward, srcs, visibility = visibility, quality_srcs = quality_srcs, **kwargs)

def d_binary(name, *, srcs = None, visibility = None, **kwargs):
    """Thin wrapper over the rules_d d_binary rule."""
    kwargs = d_kwargs(kwargs)
    quality_srcs = d_quality_srcs(srcs, kwargs.get("string_srcs"))
    kwargs = _string_srcs(name, kwargs)
    upstream = dict(kwargs, tags = dx_upstream_tags(kwargs.get("tags", None)))
    dx_wrap_binary(name, _d_binary, _d_binary_forward, srcs, visibility = visibility, upstream_kwargs = upstream, quality_srcs = quality_srcs, **kwargs)

D_COVERAGE_COLLECTOR = True

def d_test(name, *, srcs = None, visibility = None, **kwargs):
    """Thin wrapper over the rules_d d_test rule. srcs is required; -cov is added to dopts under bazel coverage."""
    if srcs == None:
        fail("d_test //" + native.package_name() + ":" + name + " needs srcs")
    kwargs = d_kwargs(kwargs)
    quality_srcs = d_quality_srcs(srcs, kwargs.get("string_srcs"))
    kwargs = _cover(_string_srcs(name, kwargs))
    dx_wrap_test(name, _d_test, _d_forward_test, srcs, visibility = visibility, quality_srcs = quality_srcs, extra_forward_kwargs = {"deps": kwargs.get("deps", [])}, coverage_collector = D_COVERAGE_COLLECTOR, **kwargs)

def d_fuzz_test(name, *, srcs = None, visibility = None, tags = None, dopts = None, linkopts = None, **kwargs):
    """A libFuzzer binary with AddressSanitizer plus a `<name>_run` runner for `dx fuzz`. srcs define LLVMFuzzerTestOneInput, for example with ldc.libfuzzer."""
    flags = ["-fsanitize=fuzzer,address"]
    kwargs["target_compatible_with"] = (kwargs.get("target_compatible_with") or []) + D_COMPATIBLE_WITH
    dx_fuzz_test(name, d_binary, {"srcs": srcs, "dopts": flags + (dopts or []), "linkopts": flags + (linkopts or [])}, tags = tags, visibility = visibility, **kwargs)

def d_bench(name, *, srcs = None, visibility = None, tags = None, **kwargs):
    """A d_binary run by `dx bench`; it writes dx-bench.json to TEST_UNDECLARED_OUTPUTS_DIR."""
    dx_bench_test(
        name,
        d_binary,
        native_test,
        kwargs,
        tags = tags,
        visibility = visibility,
        binary_kwargs = {"srcs": srcs},
        compatible_with = D_COMPATIBLE_WITH,
        runner_kwargs = {"src": ":" + name + "_bin"},
    )
