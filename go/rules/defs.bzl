"""Minimal Go wrappers."""

load("@bazel_skylib//rules:native_binary.bzl", "native_test")
load("@rules_go//go:def.bzl", _GoArchive = "GoArchive", _GoInfo = "GoInfo", _go_binary = "go_binary", _go_library = "go_library", _go_test = "go_test")
load("@rules_shell//shell:sh_test.bzl", "sh_test")
load("//cc/toolchains:bindings.bzl", "CC_TOOLCHAIN_TYPE", "cc_toolchain_info")
load("//core:fuzz.bzl", "dx_fuzz_test")
load("//core:fuzz_bench.bzl", "dx_bench_test")
load("//core:wrapper.bzl", "DX_EXEC_PROVIDES", "dx_executable_forward_rule", "dx_lcov_merger_attr", "dx_library_forward_rule", "dx_without_warnings_attr", "dx_wrap", "dx_wrap_binary", "dx_wrap_test")
load("//quality:sources.bzl", "QualitySourcesInfo", "UPSTREAM_CC_CLASS_EXTENSIONS")
load(":cgo.bzl", "cgo_toolchain_error")

_DX_GO_LIBRARY_PROVIDES = [
    _GoInfo,
    _GoArchive,
    DefaultInfo,
    InstrumentedFilesInfo,
    QualitySourcesInfo,
]

_DX_GO_SOURCE_SPECS = [("go", "go")] + [(k, UPSTREAM_CC_CLASS_EXTENSIONS[k]) for k in ("c", "cpp")]

_CGO_ATTRS = {"cgo": attr.bool(doc = "Whether the target uses cgo.")}
_CGO_TOOLCHAINS = [config_common.toolchain_type(CC_TOOLCHAIN_TYPE, mandatory = False)]

def _validate_cgo(ctx):
    if not ctx.attr.cgo:
        return
    info = cc_toolchain_info(ctx)
    if info == None:
        return
    err = cgo_toolchain_error(info.compiler, info.target_gnu_system_name)
    if err:
        fail(str(ctx.label) + ": " + err)

_go_library_forward = dx_library_forward_rule(
    provides = _DX_GO_LIBRARY_PROVIDES,
    required_providers = [(_GoInfo, "GoInfo"), (_GoArchive, "GoArchive")],
    quality_specs = _DX_GO_SOURCE_SPECS,
    what = "go_*",
    allow_files = True,
    upstream_providers = [[_GoInfo]],
    extra_attrs = _CGO_ATTRS,
    toolchains = _CGO_TOOLCHAINS,
    validate = _validate_cgo,
)

_go_binary_forward = dx_executable_forward_rule(
    kind = "executable",
    provides = DX_EXEC_PROVIDES,
    required_providers = [],
    quality_specs = _DX_GO_SOURCE_SPECS,
    what = "go_*",
    allow_files = True,
    upstream_providers = [[_GoArchive]],
    optional_providers = [_GoArchive],
    runtime = "besteffort",
    extra_attrs = _CGO_ATTRS,
    toolchains = _CGO_TOOLCHAINS,
    validate = _validate_cgo,
)

_go_forward_test = dx_executable_forward_rule(
    kind = "test",
    provides = DX_EXEC_PROVIDES,
    required_providers = [],
    quality_specs = _DX_GO_SOURCE_SPECS,
    what = "go_*",
    allow_files = True,
    upstream_providers = [[_GoArchive]],
    extra_attrs = dict(dx_lcov_merger_attr(), **_CGO_ATTRS),
    toolchains = _CGO_TOOLCHAINS,
    validate = _validate_cgo,
    optional_providers = [_GoArchive],
)

def _cgo_kwargs(kwargs):
    return {"cgo": kwargs.get("cgo", False)}

def go_library(name, srcs, importpath, visibility = None, **kwargs):
    """Minimal wrapper over go_library."""
    kwargs = dx_without_warnings_attr(kwargs)
    dx_wrap(name, _go_library, _go_library_forward, srcs, visibility = visibility, importpath = importpath, extra_forward_kwargs = _cgo_kwargs(kwargs), **kwargs)

def go_binary(name, *, srcs = None, visibility = None, **kwargs):
    """Minimal wrapper over go_binary. cgo is off unless set."""
    kwargs = dx_without_warnings_attr(kwargs)
    dx_wrap_binary(name, _go_binary, _go_binary_forward, srcs, visibility = visibility, extra_forward_kwargs = _cgo_kwargs(kwargs), **kwargs)

def go_test(name, *, srcs = None, visibility = None, **kwargs):
    """Minimal wrapper over go_test. cgo is off unless set."""
    kwargs = dx_without_warnings_attr(kwargs)
    dx_wrap_test(name, _go_test, _go_forward_test, srcs, visibility = visibility, extra_forward_kwargs = _cgo_kwargs(kwargs), **kwargs)

def go_fuzz_test(name, *, target, srcs = None, visibility = None, tags = None, **kwargs):
    """A go_test plus a `<name>_run` runner that fuzzes the function `target` for `dx fuzz`."""
    dx_fuzz_test(
        name,
        go_test,
        {"srcs": srcs},
        tags = tags,
        visibility = visibility,
        fuzz_rule = native_test,
        fuzz_kwargs = {"src": ":" + name + "_bin"},
        engine = "go",
        target = target,
        test = True,
        **kwargs
    )

def go_bench(name, *, srcs = None, visibility = None, tags = None, **kwargs):
    """A go_test of the Benchmark functions plus a `<name>` runner that writes dx-bench.json for `dx bench`."""
    dx_bench_test(
        name,
        go_test,
        sh_test,
        kwargs,
        tags = tags,
        visibility = visibility,
        binary_kwargs = {"srcs": srcs},
        runner_kwargs = {"srcs": [Label("//go/rules:bench_run.sh")]},
        pass_binary = True,
        argv = ["-test.bench=.", "-test.run=^$", "-test.benchmem", "-test.count=1"],
    )
