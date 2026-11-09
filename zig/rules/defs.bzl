"""Minimal Zig wrappers."""

load("@rules_shell//shell:sh_test.bzl", "sh_test")
load("@rules_zig//zig:defs.bzl", _zig_binary = "zig_binary", _zig_configure_binary = "zig_configure_binary", _zig_library = "zig_library", _zig_test = "zig_test")
load("@rules_zig//zig/private/providers:zig_module_info.bzl", _ZigModuleInfo = "ZigModuleInfo")
load("//libs/starlark:entry.bzl", "dx_entry_quality_srcs", "dx_infer_main", "dx_main_candidates")
load("//libs/starlark:fuzz_bench.bzl", "merge_tags", "split_test_kwargs", "with_default_size")
load("//libs/starlark:wrapper.bzl", "DX_EXEC_PROVIDES", "dx_executable_forward_rule", "dx_lcov_merger_attr", "dx_library_forward_rule", "dx_upstream_tags", "dx_wrap", "dx_wrap_binary", "dx_wrap_test")
load("//quality:sources.bzl", "QualitySourcesInfo", "QualityToolsInfo")
load("//tools/coverage/kcov:kcov.bzl", "dx_kcov_attrs", "dx_kcov_test_coverage")

# Set False to tag zig tests dx-no-coverage-collector instead of running kcov.
ZIG_COVERAGE_COLLECTOR = True

_DX_ZIG_LIBRARY_PROVIDES = [
    _ZigModuleInfo,
    DefaultInfo,
    InstrumentedFilesInfo,
    QualitySourcesInfo,
    QualityToolsInfo,
]

ZIG_QUALITY_TOOLS = {
    "zig_ast_check": Label("//zig/tools:zig"),
    "zig_fmt": Label("//zig/tools:zig"),
}
_DX_ZIG_EXEC_PROVIDES = DX_EXEC_PROVIDES + [QualityToolsInfo]
_DX_ZIG_SOURCE_SPECS = [("zig", "zig")]
_DX_ZIG_SOURCE_EXTS = [".zig"]

_zig_library_forward = dx_library_forward_rule(
    provides = _DX_ZIG_LIBRARY_PROVIDES,
    required_providers = [(_ZigModuleInfo, "ZigModuleInfo")],
    quality_specs = _DX_ZIG_SOURCE_SPECS,
    what = "zig_*",
    allow_files = _DX_ZIG_SOURCE_EXTS,
    upstream_providers = [[_ZigModuleInfo]],
    runtime = "besteffort",
    quality_tools = ZIG_QUALITY_TOOLS,
    instrument_srcs = True,
)

def _no_coverage_impl(_settings, _attr):
    """Builds the Zig upstream without coverage instrumentation."""
    return {"//command_line_option:collect_code_coverage": False}

_no_coverage = transition(
    implementation = _no_coverage_impl,
    inputs = [],
    outputs = ["//command_line_option:collect_code_coverage"],
)

def _kcov_upstream_impl(settings, _attr):
    """Builds the Zig test upstream for kcov under coverage: LLVM DWARF, unstripped, uninstrumented."""
    coverage = settings["//command_line_option:collect_code_coverage"]
    return {
        "//command_line_option:collect_code_coverage": False,
        "//command_line_option:strip": "never" if coverage else settings["//command_line_option:strip"],
        "//zig/rules:kcov": coverage,
    }

_kcov_upstream = transition(
    implementation = _kcov_upstream_impl,
    inputs = [
        "//command_line_option:collect_code_coverage",
        "//command_line_option:strip",
    ],
    outputs = [
        "//command_line_option:collect_code_coverage",
        "//command_line_option:strip",
        "//zig/rules:kcov",
    ],
)

_zig_binary_forward = dx_executable_forward_rule(
    kind = "executable",
    provides = _DX_ZIG_EXEC_PROVIDES,
    required_providers = [],
    quality_specs = _DX_ZIG_SOURCE_SPECS,
    what = "zig_*",
    allow_files = _DX_ZIG_SOURCE_EXTS,
    upstream_providers = None,
    runtime = "besteffort",
    quality_tools = ZIG_QUALITY_TOOLS,
    upstream_cfg = _no_coverage,
)

def _zig_test_coverage(ctx, upstream):
    """Runs the upstream Zig test under kcov on Linux."""
    deps = [dep[_ZigModuleInfo].transitive_inputs for dep in ctx.attr.deps if _ZigModuleInfo in dep]
    sources = [f for f in depset(ctx.files.srcs, transitive = deps).to_list() if f.extension == "zig"]
    return dx_kcov_test_coverage(ctx, upstream, sources, ["upstream", "deps"])

_zig_forward_test = dx_executable_forward_rule(
    kind = "test",
    provides = _DX_ZIG_EXEC_PROVIDES,
    required_providers = [],
    quality_specs = _DX_ZIG_SOURCE_SPECS,
    what = "zig_*",
    allow_files = _DX_ZIG_SOURCE_EXTS,
    upstream_providers = None,
    extra_attrs = dx_lcov_merger_attr() | dx_kcov_attrs() | {
        "deps": attr.label_list(doc = "Dependencies of the wrapped test, whose sources count for coverage."),
    },
    runtime = "besteffort",
    quality_tools = ZIG_QUALITY_TOOLS,
    upstream_cfg = _kcov_upstream,
    coverage_providers = _zig_test_coverage,
)

def zig_import_name(name, import_name):
    """Returns the module import name, defaulting to the public target name."""
    return import_name if import_name else name

def zig_library(name, main, *, srcs = None, import_name = None, visibility = None, **kwargs):
    """Minimal wrapper over zig_library; @import uses the public name."""
    dx_wrap(name, _zig_library, _zig_library_forward, srcs, visibility = visibility, quality_srcs = dx_entry_quality_srcs(main, srcs), main = main, import_name = zig_import_name(name, import_name), **kwargs)

def zig_binary(name, *, main = None, srcs = None, visibility = None, **kwargs):
    """Minimal wrapper over zig_binary. main is name.zig, else the single src."""
    main = dx_infer_main("zig_binary", name, main, srcs, dx_main_candidates(name, ".zig"))
    upstream = dict(kwargs, main = main, tags = dx_upstream_tags(kwargs.get("tags", None)))
    dx_wrap_binary(name, _zig_binary, _zig_binary_forward, srcs, visibility = visibility, upstream_kwargs = upstream, quality_srcs = dx_entry_quality_srcs(main, srcs), main = main, **kwargs)

def zig_test(name, *, main = None, srcs = None, visibility = None, **kwargs):
    """Minimal wrapper over zig_test. main is name.zig, else the single src; kcov builds add -fllvm."""
    main = dx_infer_main("zig_test", name, main, srcs, dx_main_candidates(name, ".zig"))
    upstream = dict(kwargs, main = main, zigopts = kwargs.get("zigopts", []) + select({
        Label("//zig/rules:kcov_build"): ["-fllvm"],
        "//conditions:default": [],
    }))
    dx_wrap_test(name, _zig_test, _zig_forward_test, srcs, visibility = visibility, upstream_kwargs = upstream, quality_srcs = dx_entry_quality_srcs(main, srcs), main = main, extra_forward_kwargs = {"deps": kwargs.get("deps", [])}, coverage_collector = ZIG_COVERAGE_COLLECTOR, **kwargs)

def zig_bench(name, *, main = None, srcs = None, visibility = None, tags = None, **kwargs):
    """A ReleaseFast zig_binary plus a `<name>` runner that writes dx-bench.json for `dx bench`."""
    binary = name + "_bin"
    build, run = split_test_kwargs(kwargs)
    run = with_default_size(run)
    zig_binary(
        name = binary + "_unoptimized",
        main = main,
        srcs = srcs,
        visibility = ["//visibility:private"],
        testonly = True,
        tags = merge_tags(tags, "manual"),
        **build
    )
    _zig_configure_binary(
        name = binary,
        actual = ":" + binary + "_unoptimized",
        mode = "release_fast",
        visibility = ["//visibility:private"],
        testonly = True,
        tags = merge_tags(tags, "manual"),
    )
    sh_test(
        name = name,
        srcs = [Label("//zig/rules:bench_run.sh")],
        args = ["$(rootpath :" + binary + ")"] + run.pop("args", []),
        data = run.pop("data", []) + [":" + binary],
        tags = merge_tags(tags, "dx-bench", "manual"),
        testonly = True,
        visibility = visibility,
        **run
    )
