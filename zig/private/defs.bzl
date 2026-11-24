"""Minimal Zig wrappers."""

load("@rules_shell//shell:sh_test.bzl", "sh_test")
load("@rules_zig//zig:defs.bzl", _zig_binary = "zig_binary", _zig_configure_binary = "zig_configure_binary", _zig_library = "zig_library", _zig_test = "zig_test")
load("@rules_zig//zig/private/providers:zig_module_info.bzl", _ZigModuleInfo = "ZigModuleInfo")
load("//core:entry.bzl", "dx_entry_quality_srcs", "dx_infer_main", "dx_main_candidates")
load("//core:fuzz_bench.bzl", "dx_bench_test")
load("//core:wrapper.bzl", "DX_EXEC_PROVIDES", "dx_executable_forward_rule", "dx_forward_spec", "dx_language_spec", "dx_lcov_merger_attr", "dx_library_forward_rule", "dx_upstream_tags", "dx_without_warnings_attr", "dx_wrap", "dx_wrap_binary", "dx_wrap_test")
load("//core/quality:sources.bzl", "QualitySourcesInfo", "QualityToolsInfo", "single_target")
load("//tools/coverage/kcov:kcov.bzl", "dx_kcov_attrs", "dx_kcov_test_coverage")
load("//tools/coverage/sancov:sancov.bzl", "dx_sancov_attrs", "dx_sancov_test_coverage")
load(":mode.bzl", "ZigOptsInfo", "ZigReleaseFastInfo", "zig_opts_aspect")

ZIG_COVERAGE_COLLECTOR = True

_DX_ZIG_LIBRARY_PROVIDES = [
    _ZigModuleInfo,
    DefaultInfo,
    InstrumentedFilesInfo,
    QualitySourcesInfo,
    QualityToolsInfo,
]

ZIG_QUALITY_TOOLS = {
    "zig_ast_check": Label("//zig/private/tools:zig"),
    "zig_fmt": Label("//zig/private/tools:zig"),
}
_DX_ZIG_EXEC_PROVIDES = DX_EXEC_PROVIDES + [QualityToolsInfo]
_DX_ZIG_SOURCE_SPECS = [("zig", "zig")]
_DX_ZIG_SOURCE_EXTS = [".zig"]

_ZIG = dx_language_spec(what = "zig_*", quality_specs = _DX_ZIG_SOURCE_SPECS, allow_files = _DX_ZIG_SOURCE_EXTS, quality_tools = ZIG_QUALITY_TOOLS)

_zig_library_forward = dx_library_forward_rule(
    _ZIG,
    dx_forward_spec(
        provides = _DX_ZIG_LIBRARY_PROVIDES,
        required_providers = [(_ZigModuleInfo, "ZigModuleInfo")],
        upstream_providers = [[_ZigModuleInfo]],
        runtime = "besteffort",
        instrument_srcs = True,
    ),
)

_ZIG_MODE = str(Label("@rules_zig//zig/settings:mode"))

def _upstream_mode(settings, coverage = False):
    """Returns the set Zig mode, else ReleaseSafe, or Debug under coverage so the collectors keep line data."""
    mode = settings[_ZIG_MODE]
    if mode != "auto":
        return mode
    return "debug" if coverage else "release_safe"

def _validate_release_fast(ctx):
    """Fails on ReleaseFast, from the mode or the zigopts of the target or its zig_library deps, unless the target sets allow_release_fast."""
    if ctx.attr.allow_release_fast:
        return
    if single_target(ctx.attr._release_fast)[ZigReleaseFastInfo].value:
        fail("Zig mode release_fast drops safety checks; set allow_release_fast = True on the target to use it")
    if single_target(ctx.attr.upstream)[ZigOptsInfo].release_fast:
        fail("{}: zigopts set ReleaseFast or read an @file, on the target or a zig_library it depends on, which can drop safety checks; set allow_release_fast = True on the target to use it".format(ctx.label))

def _zig_binary_upstream_impl(settings, _attr):
    """Builds the Zig binary upstream without coverage, in ReleaseSafe unless a mode is set."""
    return {
        "//command_line_option:collect_code_coverage": False,
        _ZIG_MODE: _upstream_mode(settings),
    }

_zig_binary_upstream = transition(
    implementation = _zig_binary_upstream_impl,
    inputs = [_ZIG_MODE],
    outputs = ["//command_line_option:collect_code_coverage", _ZIG_MODE],
)

def _zig_test_upstream_impl(settings, _attr):
    """Builds the Zig test upstream in ReleaseSafe, and for kcov or sancov under coverage: Debug, LLVM DWARF, unstripped."""
    coverage = settings["//command_line_option:collect_code_coverage"]
    return {
        "//command_line_option:collect_code_coverage": False,
        "//command_line_option:strip": "never" if coverage else settings["//command_line_option:strip"],
        "//zig:kcov": coverage,
        _ZIG_MODE: _upstream_mode(settings, coverage),
    }

_zig_test_upstream = transition(
    implementation = _zig_test_upstream_impl,
    inputs = [
        "//command_line_option:collect_code_coverage",
        "//command_line_option:strip",
        _ZIG_MODE,
    ],
    outputs = [
        "//command_line_option:collect_code_coverage",
        "//command_line_option:strip",
        "//zig:kcov",
        _ZIG_MODE,
    ],
)

def _release_mode_attrs(upstream_cfg):
    """Returns the ReleaseFast opt-in attr and the mode probe, built in the upstream configuration."""
    return {
        "allow_release_fast": attr.bool(doc = "Allow ReleaseFast, which drops the safety checks that ReleaseSafe keeps."),
        "_release_fast": attr.label(default = Label("//zig/private:release_fast"), cfg = upstream_cfg, providers = [ZigReleaseFastInfo]),
    }

_zig_binary_forward = dx_executable_forward_rule(
    _ZIG,
    "executable",
    dx_forward_spec(
        provides = _DX_ZIG_EXEC_PROVIDES,
        extra_attrs = _release_mode_attrs(_zig_binary_upstream),
        upstream_cfg = _zig_binary_upstream,
        upstream_aspects = [zig_opts_aspect],
        validate = _validate_release_fast,
    ),
)

def _zig_test_coverage(ctx, upstream):
    """Runs the upstream Zig test under kcov on Linux, or reads its sancov PCs on macOS."""
    deps = [dep[_ZigModuleInfo].transitive_inputs for dep in ctx.attr.deps if _ZigModuleInfo in dep]
    sources = [f for f in depset(ctx.files.srcs, transitive = deps).to_list() if f.extension == "zig"]
    kcov = dx_kcov_test_coverage(ctx, upstream, sources, ["upstream", "deps"])
    if kcov:
        return kcov
    return dx_sancov_test_coverage(ctx, upstream, sources, ["upstream", "deps"])

_zig_forward_test = dx_executable_forward_rule(
    _ZIG,
    "test",
    dx_forward_spec(
        provides = _DX_ZIG_EXEC_PROVIDES,
        extra_attrs = dx_lcov_merger_attr() | dx_kcov_attrs() | dx_sancov_attrs() | _release_mode_attrs(_zig_test_upstream) | {
            "deps": attr.label_list(doc = "Dependencies of the wrapped test, whose sources count for coverage."),
        },
        upstream_cfg = _zig_test_upstream,
        upstream_aspects = [zig_opts_aspect],
        validate = _validate_release_fast,
        coverage_providers = _zig_test_coverage,
    ),
)

def zig_import_name(name, import_name):
    """Returns the module import name, defaulting to the public target name."""
    return import_name if import_name else name

def zig_library(name, main, *, srcs = None, import_name = None, visibility = None, **kwargs):
    """Minimal wrapper over zig_library; @import uses the public name."""
    kwargs = dx_without_warnings_attr(kwargs)
    dx_wrap(name, _zig_library, _zig_library_forward, srcs, visibility = visibility, quality_srcs = dx_entry_quality_srcs(main, srcs), main = main, import_name = zig_import_name(name, import_name), **kwargs)

def zig_binary(name, *, srcs = None, main = None, visibility = None, allow_release_fast = False, **kwargs):
    """Minimal wrapper over zig_binary in ReleaseSafe. main is name.zig, else the single src."""
    kwargs = dx_without_warnings_attr(kwargs)
    main = dx_infer_main("zig_binary", name, main, srcs, dx_main_candidates(name, ".zig"))
    upstream = dict(kwargs, main = main, tags = dx_upstream_tags(kwargs.get("tags", None)), zigopts = ["-j1"] + kwargs.get("zigopts", []))
    dx_wrap_binary(name, _zig_binary, _zig_binary_forward, srcs, visibility = visibility, upstream_kwargs = upstream, extra_forward_kwargs = {"allow_release_fast": allow_release_fast}, quality_srcs = dx_entry_quality_srcs(main, srcs), main = main, **kwargs)

def zig_test(name, *, srcs = None, main = None, visibility = None, allow_release_fast = False, **kwargs):
    """Minimal wrapper over zig_test in ReleaseSafe. main is name.zig, else the single src; coverage builds add -fllvm, and -ffuzz on macOS."""
    kwargs = dx_without_warnings_attr(kwargs)
    main = dx_infer_main("zig_test", name, main, srcs, dx_main_candidates(name, ".zig"))
    upstream = dict(
        kwargs,
        main = main,
        zigopts = ["-j1"] + kwargs.get("zigopts", []) + select({
            Label("//zig:sancov_build"): ["-fllvm", "-ffuzz"],
            Label("//zig:kcov_build"): ["-fllvm"],
            "//conditions:default": [],
        }),
        test_runner = select({
            Label("//zig:sancov_build"): Label("//zig/private:coverage_runner.zig"),
            "//conditions:default": kwargs.get("test_runner"),
        }),
    )
    dx_wrap_test(name, _zig_test, _zig_forward_test, srcs, visibility = visibility, upstream_kwargs = upstream, quality_srcs = dx_entry_quality_srcs(main, srcs), main = main, extra_forward_kwargs = {"allow_release_fast": allow_release_fast, "deps": kwargs.get("deps", [])}, coverage_collector = ZIG_COVERAGE_COLLECTOR, **kwargs)

def _zig_bench_binary(name, tags = None, **kwargs):
    """A ReleaseFast zig_binary under `name`."""
    zig_binary(
        name = name + "_unoptimized",
        tags = tags,
        allow_release_fast = True,
        **kwargs
    )
    _zig_configure_binary(
        name = name,
        actual = ":" + name + "_unoptimized",
        mode = "release_fast",
        visibility = ["//visibility:private"],
        testonly = True,
        tags = tags,
    )

def zig_bench(name, *, srcs = None, main = None, visibility = None, tags = None, **kwargs):
    """A ReleaseFast zig_binary plus a `<name>` runner that writes dx-bench.json for `dx bench`."""
    dx_bench_test(
        name,
        _zig_bench_binary,
        sh_test,
        kwargs,
        tags = tags,
        visibility = visibility,
        binary_kwargs = {"main": main, "srcs": srcs},
        runner_kwargs = {"srcs": [Label("//zig/private:bench_run.sh")]},
        pass_binary = True,
    )
