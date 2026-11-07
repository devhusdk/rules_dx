"""Minimal Go wrappers."""

load("@rules_shell//shell:sh_binary.bzl", "sh_binary")
load("@rules_shell//shell:sh_test.bzl", "sh_test")
load("@rules_go//go:def.bzl", _GoArchive = "GoArchive", _GoInfo = "GoInfo", _go_binary = "go_binary", _go_library = "go_library", _go_test = "go_test")
load(":cgo.bzl", "cgo_toolchain_error")
load("//cc/toolchains:bindings.bzl", "CC_TOOLCHAIN_TYPE", "cc_toolchain_info")
load("//libs/starlark:wrapper.bzl", "DX_EXEC_PROVIDES", "dx_executable_forward_rule", "dx_lcov_merger_attr", "dx_library_forward_rule", "dx_wrap", "dx_wrap_binary", "dx_wrap_test")
load("//quality:sources.bzl", "QualitySourcesInfo", "UPSTREAM_CC_CLASS_EXTENSIONS")

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
    dx_wrap(name, _go_library, _go_library_forward, srcs, visibility = visibility, importpath = importpath, extra_forward_kwargs = _cgo_kwargs(kwargs), **kwargs)

def go_binary(name, srcs = None, visibility = None, **kwargs):
    """Minimal wrapper over go_binary."""
    dx_wrap_binary(name, _go_binary, _go_binary_forward, srcs, visibility = visibility, extra_forward_kwargs = _cgo_kwargs(kwargs), **kwargs)

def go_test(name, srcs = None, visibility = None, **kwargs):
    """Minimal wrapper over go_test."""
    dx_wrap_test(name, _go_test, _go_forward_test, srcs, visibility = visibility, extra_forward_kwargs = _cgo_kwargs(kwargs), **kwargs)

def go_fuzz_test(name, fuzz, srcs = None, visibility = None, tags = None, **kwargs):
    """A go_test plus a `<name>_run` runner that fuzzes the function `fuzz` for `dx fuzz`."""
    go_test(name = name, srcs = srcs, visibility = visibility, tags = (tags or []) + ["dx-fuzz"], **kwargs)
    sh_binary(
        name = name + "_run",
        srcs = [Label("//go/rules:fuzz_run.sh")],
        args = ["$(rootpath :" + name + ")", fuzz],
        data = [":" + name],
        testonly = True,
        visibility = visibility,
    )

def go_benchmark(name, srcs = None, visibility = None, tags = None, **kwargs):
    """A go_test of the Benchmark functions plus a `<name>` runner that writes dx-bench.json for `dx bench`."""
    go_test(
        name = name + "_bin",
        srcs = srcs,
        visibility = visibility,
        tags = (tags or []) + ["manual"],
        **kwargs
    )
    sh_test(
        name = name,
        srcs = [Label("//go/rules:bench_run.sh")],
        args = ["$(rootpath :" + name + "_bin)", "-test.bench=.", "-test.run=^$", "-test.benchmem", "-test.count=1"],
        data = [":" + name + "_bin"],
        tags = (tags or []) + ["dx-bench", "manual"],
        testonly = True,
        visibility = visibility,
    )
