"""Thin dx wrappers over the owned PHP rules."""

load("//core:entry.bzl", "dx_entry_quality_srcs", "dx_infer_main", "dx_main_candidates")
load("//core:fuzz_bench.bzl", "merge_tags", "with_default_size")
load("//core:platform_support.bzl", "cells_support", "compatible_with")
load("//core:wrapper.bzl", "DX_EXEC_PROVIDES", "dx_executable_forward_rule", "dx_language_kwargs", "dx_lcov_merger_attr", "dx_library_forward_rule", "dx_wrap", "dx_wrap_binary", "dx_wrap_test")
load("//php/private/toolchain:cells.bzl", "PHP_CELLS")
load("//quality:sources.bzl", "QualitySourcesInfo")
load(":php.bzl", "PHP_SOURCE_EXTS", _php_binary = "php_binary", _php_library = "php_library", _php_test = "php_test")
load(":providers.bzl", _PhpInfo = "PhpInfo")

PhpInfo = _PhpInfo

PHP_SOURCE_SPECS = [("php", ["php"])]

PLATFORM_SUPPORT = cells_support("//php:defs.bzl", "PHP_COMPATIBLE_WITH", PHP_CELLS, sorted(PHP_CELLS.keys()), "//php/private/toolchain")

PHP_COMPATIBLE_WITH = compatible_with(PLATFORM_SUPPORT)

PHP_DEFERRED_SKIP_TAGS = ["no-format"]

_php_library_forward = dx_library_forward_rule(
    provides = [_PhpInfo, DefaultInfo, InstrumentedFilesInfo, QualitySourcesInfo],
    required_providers = [(_PhpInfo, "PhpInfo")],
    quality_specs = PHP_SOURCE_SPECS,
    what = "php_library",
    allow_files = PHP_SOURCE_EXTS,
    upstream_providers = [[_PhpInfo]],
    runtime = "besteffort",
)

_php_forward_binary = dx_executable_forward_rule(
    kind = "executable",
    provides = DX_EXEC_PROVIDES,
    required_providers = [],
    quality_specs = PHP_SOURCE_SPECS,
    what = "php_binary",
    allow_files = PHP_SOURCE_EXTS,
    upstream_providers = None,
    quiet_providers = [_PhpInfo],
    runtime = "besteffort",
)

_php_forward_test = dx_executable_forward_rule(
    kind = "test",
    provides = DX_EXEC_PROVIDES,
    required_providers = [],
    quality_specs = PHP_SOURCE_SPECS,
    what = "php_test",
    allow_files = PHP_SOURCE_EXTS,
    upstream_providers = None,
    extra_attrs = dx_lcov_merger_attr(),
    runtime = "besteffort",
)

def php_library(name, srcs = None, visibility = None, **kwargs):
    """PHP sources other targets load through the Composer classmap; every source is linted with php -l."""
    kwargs = dx_language_kwargs(kwargs, PHP_DEFERRED_SKIP_TAGS, PHP_COMPATIBLE_WITH)
    dx_wrap(name, _php_library, _php_library_forward, srcs, visibility = visibility, quality_srcs = dx_entry_quality_srcs(None, srcs, kwargs.get("autoload_files")), **kwargs)

def php_binary(name, *, srcs = None, main = None, visibility = None, **kwargs):
    """Runs main on the pinned PHP with the classes of deps autoloaded. main is name.php, else the single src."""
    main = dx_infer_main("php_binary", name, main, srcs, dx_main_candidates(name, ".php"))
    kwargs = dx_language_kwargs(kwargs, PHP_DEFERRED_SKIP_TAGS, PHP_COMPATIBLE_WITH)
    upstream = dict(kwargs, main = main)
    dx_wrap_binary(name, _php_binary, _php_forward_binary, srcs, visibility = visibility, upstream_kwargs = upstream, quality_srcs = dx_entry_quality_srcs(main, srcs), **kwargs)

def php_test(name, *, srcs = None, visibility = None, **kwargs):
    """Runs PHPUnit test files on the pinned PHP and PHPUnit. srcs are the test files and are linted."""
    kwargs = dx_language_kwargs(kwargs, PHP_DEFERRED_SKIP_TAGS, PHP_COMPATIBLE_WITH)
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
        tags = merge_tags(tags, "dx-bench", "manual"),
        **with_default_size(kwargs)
    )
