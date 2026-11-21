"""Thin dx wrappers over the owned PHP rules."""

load("//core:entry.bzl", "dx_entry_quality_srcs", "dx_infer_main", "dx_main_candidates")
load("//core:platform_support.bzl", "cells_support", "compatible_with")
load("//core:wrapper.bzl", "dx_executable_forward_rule", "dx_forward_spec", "dx_language_kwargs", "dx_language_spec", "dx_lcov_merger_attr", "dx_library_forward_rule", "dx_wrap", "dx_wrap_binary", "dx_wrap_test")
load("//core/quality:sources.bzl", "QualitySourcesInfo")
load("//php/private/toolchain:cells.bzl", "PHP_CELLS")
load("//php/private/coverage:coverage.bzl", "PhpPcovInfo", "coverage_descriptor")
load(":php.bzl", "NATIVE_LAUNCHER_ATTRS", "PHP_SOURCE_EXTS", "no_coverage_transition", "runfiles_path", "write_native_launcher", _php_binary = "php_binary", _php_library = "php_library", _php_test = "php_test")
load(":providers.bzl", "PhpTestInfo", _PhpInfo = "PhpInfo")

PhpInfo = _PhpInfo

PHP_SOURCE_SPECS = [("php", ["php"])]

PLATFORM_SUPPORT = cells_support("//php/private:defs.bzl", "PHP_COMPATIBLE_WITH", PHP_CELLS, sorted(PHP_CELLS), "//php/private/toolchain", unsupported = "php")

PHP_COMPATIBLE_WITH = compatible_with(PLATFORM_SUPPORT)

_PHP = dx_language_spec(what = "php_*", quality_specs = PHP_SOURCE_SPECS, allow_files = PHP_SOURCE_EXTS)

_php_library_forward = dx_library_forward_rule(
    _PHP,
    dx_forward_spec(
        provides = [_PhpInfo, DefaultInfo, InstrumentedFilesInfo, QualitySourcesInfo],
        required_providers = [(_PhpInfo, "PhpInfo")],
        upstream_providers = [[_PhpInfo]],
        runtime = "besteffort",
    ),
)

_php_forward_binary = dx_executable_forward_rule(
    _PHP,
    "executable",
    dx_forward_spec(
        quiet_providers = [_PhpInfo],
    ),
)

def _php_test_coverage(ctx, upstream):
    """Runs the upstream PHPUnit tokens under dx-runtime's php_coverage helper with PCOV loaded."""
    if PhpPcovInfo not in ctx.attr._php_pcov:
        fail(str(ctx.label) + ": the target platform has no PHP that loads PCOV")
    pcov = ctx.attr._php_pcov[PhpPcovInfo]
    test = upstream[PhpTestInfo]
    runner = ctx.executable._coverage_run
    path = lambda f: runfiles_path(ctx.workspace_name, f.short_path)
    descriptor = coverage_descriptor(
        path(runner),
        test.autoload_root,
        path(pcov.pcov),
        path(pcov.interpreter),
        test.ini,
        test.tokens,
        [path(f) for f in pcov.extensions],
    )
    # The _upstream suffix makes dx_symlink_executable_name link it as <name> or <name>.exe even when <name> holds a dot.
    # The directory must not start with "<name>.": the conformance runner copies such siblings as files.
    launcher, _, runfiles = write_native_launcher(ctx, descriptor, launcher_name = "_" + ctx.label.name + ".coverage/" + ctx.label.name + "_upstream")
    runfiles = runfiles.merge_all([
        ctx.runfiles(files = [pcov.pcov, runner] + pcov.extensions, transitive_files = pcov.files),
        ctx.attr._coverage_run[0][DefaultInfo].default_runfiles,
    ])
    return struct(
        instrumented = upstream[InstrumentedFilesInfo],
        env = upstream[RunEnvironmentInfo] if RunEnvironmentInfo in upstream else RunEnvironmentInfo(),
        runfiles = runfiles,
        executable = launcher,
    )

_php_forward_test = dx_executable_forward_rule(
    _PHP,
    "test",
    dx_forward_spec(
        extra_attrs = dx_lcov_merger_attr() | NATIVE_LAUNCHER_ATTRS | {
            "_coverage_run": attr.label(
                default = Label("@dx_prebuilt//:runtime"),
                executable = True,
                cfg = no_coverage_transition,
                doc = "dx-runtime, whose php_coverage helper runs PHPUnit under PCOV and converts its Clover report to LCOV.",
            ),
            "_php_pcov": attr.label(
                default = Label("//php/private/coverage:runtime"),
                doc = "The PHP that loads PCOV, under coverage only.",
            ),
        },
        coverage_providers = _php_test_coverage,
    ),
)

def php_library(name, srcs = None, visibility = None, **kwargs):
    """PHP sources other targets load through the Composer classmap; every source is linted with php -l."""
    kwargs = dx_language_kwargs(kwargs, compatible_with = PHP_COMPATIBLE_WITH)
    dx_wrap(name, _php_library, _php_library_forward, srcs, visibility = visibility, quality_srcs = dx_entry_quality_srcs(None, srcs, kwargs.get("autoload_files")), **kwargs)

def php_binary(name, *, srcs = None, main = None, visibility = None, **kwargs):
    """Runs main on the pinned PHP with the classes of deps autoloaded. main is name.php, else the single src."""
    main = dx_infer_main("php_binary", name, main, srcs, dx_main_candidates(name, ".php"))
    kwargs = dx_language_kwargs(kwargs, compatible_with = PHP_COMPATIBLE_WITH)
    upstream = dict(kwargs, main = main)
    dx_wrap_binary(name, _php_binary, _php_forward_binary, srcs, visibility = visibility, upstream_kwargs = upstream, quality_srcs = dx_entry_quality_srcs(main, srcs), **kwargs)

def php_test(name, *, srcs = None, visibility = None, **kwargs):
    """Runs PHPUnit test files on the pinned PHP and PHPUnit. srcs are the test files and are linted."""
    kwargs = dx_language_kwargs(kwargs, compatible_with = PHP_COMPATIBLE_WITH)
    dx_wrap_test(name, _php_test, _php_forward_test, srcs, visibility = visibility, quality_srcs = dx_entry_quality_srcs(None, srcs), **kwargs)

def php_bench(name, *, srcs, data = None, env = None, tags = None, **kwargs):
    """A php_test run by dx bench: phpbench runs the `srcs` files, one class per file named like the file, and the result is written as dx-bench.json."""
    phar = Label("@dx_phpbench//:phpbench.phar")
    php_test(
        name = name,
        srcs = [Label("//php/private:BenchTest.php")],
        data = (data or []) + srcs + [phar],
        env = dict(
            env or {},
            DX_BENCH_FILES = " ".join(["$(rlocationpath %s)" % bench for bench in srcs]),
            DX_PHPBENCH = "$(rlocationpath %s)" % phar,
        ),
        tags = list(tags or []) + ([] if "dx-bench" in (tags or []) else ["dx-bench"]),
        **dict(kwargs, size = kwargs.get("size", "small"))
    )
