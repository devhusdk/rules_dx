"""Thin dx wrappers over the owned PHP rules."""

load("//libs/starlark:wrapper.bzl", "DX_EXEC_PROVIDES", "dx_executable_forward_rule", "dx_lcov_merger_attr", "dx_library_forward_rule", "dx_wrap", "dx_wrap_binary", "dx_wrap_test")
load("//quality:sources.bzl", "QualitySourcesInfo")
load(":php.bzl", "PHP_SOURCE_EXTS", _PhpInfo = "PhpInfo", _php_binary = "php_binary", _php_library = "php_library", _php_test = "php_test")

PhpInfo = _PhpInfo

PHP_SOURCE_SPECS = [("php", ["php"])]

PHP_COMPATIBLE_WITH = [
    Label("@platforms//os:linux"),
    Label("@platforms//cpu:x86_64"),
]

PHP_DEFERRED_SKIP_TAGS = ["no-format", "no-lint", "no-typecheck"]

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

def php_quality_srcs(main, srcs, extra = None):
    """Returns the hand-written sources a PHP target owns: main first, then srcs, then extra, each once."""
    owned = []
    for src in ([main] if main != None else []) + list(srcs or []) + list(extra or []):
        if src not in owned:
            owned.append(src)
    return owned

def php_compatible_with(target_compatible_with):
    """Appends the pinned PHP cells to the caller's constraints."""
    return (target_compatible_with or []) + PHP_COMPATIBLE_WITH

def php_deferred_tags(tags):
    """Adds the skip tags that keep the deferred php class out of quality aspects."""
    out = list(tags or [])
    for tag in PHP_DEFERRED_SKIP_TAGS:
        if tag not in out:
            out.append(tag)
    return out

def php_kwargs(kwargs):
    """Returns kwargs with the deferred quality tags and the PHP cell constraints."""
    return dict(
        kwargs,
        tags = php_deferred_tags(kwargs.get("tags")),
        target_compatible_with = php_compatible_with(kwargs.get("target_compatible_with")),
    )

def php_library(name, srcs = None, visibility = None, **kwargs):
    """PHP sources other targets load through the Composer classmap; every source is linted with php -l."""
    kwargs = php_kwargs(kwargs)
    srcs = srcs or []
    dx_wrap(name, _php_library, _php_library_forward, srcs, visibility = visibility, quality_srcs = php_quality_srcs(None, srcs, kwargs.get("autoload_files")), **kwargs)

def php_binary(name, main, srcs = None, visibility = None, **kwargs):
    """Runs main on the pinned PHP with the classes of deps autoloaded."""
    kwargs = php_kwargs(kwargs)
    upstream = dict(kwargs, main = main)
    dx_wrap_binary(name, _php_binary, _php_forward_binary, srcs, visibility = visibility, upstream_kwargs = upstream, quality_srcs = php_quality_srcs(main, srcs), **kwargs)

def php_test(name, srcs, visibility = None, **kwargs):
    """Runs PHPUnit test files on the pinned PHP and PHPUnit."""
    kwargs = php_kwargs(kwargs)
    dx_wrap_test(name, _php_test, _php_forward_test, srcs, visibility = visibility, quality_srcs = php_quality_srcs(None, srcs), **kwargs)
