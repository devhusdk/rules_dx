"""Thin Perl wrappers over rules_perl."""

load("@rules_perl//perl:defs.bzl", _PerlInfo = "PerlInfo", _perl_binary = "perl_binary", _perl_library = "perl_library", _perl_test = "perl_test")
load("//libs/starlark:wrapper.bzl", "DX_EXEC_PROVIDES", "dx_executable_forward_rule", "dx_lcov_merger_attr", "dx_library_forward_rule", "dx_wrap", "dx_wrap_binary", "dx_wrap_test")
load("//quality:sources.bzl", "QualitySourcesInfo", "QualityToolsInfo")

PerlInfo = _PerlInfo

PERL_COVERAGE_COLLECTOR = False

PERL_QUALITY_TOOLS = {
    "perlcritic": Label("//perl/tools:perlcritic"),
    "perltidy": Label("//perl/tools:perltidy"),
}

_PERL_EXEC_PROVIDES = DX_EXEC_PROVIDES + [QualityToolsInfo]

PERL_SOURCE_SPECS = [("perl", ["pl", "pm", "t"])]

PERL_SOURCE_EXTS = [".pl", ".pm", ".t"]

PERL_COMPATIBLE_WITH = select({
    Label("//perl/rules:rules_perl_platform"): [],
    "//conditions:default": [Label("@platforms//:incompatible")],
})

_perl_library_forward = dx_library_forward_rule(
    provides = [_PerlInfo, DefaultInfo, InstrumentedFilesInfo, QualitySourcesInfo, QualityToolsInfo],
    required_providers = [(_PerlInfo, "PerlInfo")],
    quality_specs = PERL_SOURCE_SPECS,
    what = "perl_library",
    allow_files = PERL_SOURCE_EXTS,
    upstream_providers = [[_PerlInfo]],
    runtime = "besteffort",
    quality_tools = PERL_QUALITY_TOOLS,
)

_perl_binary_forward = dx_executable_forward_rule(
    kind = "executable",
    provides = _PERL_EXEC_PROVIDES,
    required_providers = [],
    quality_specs = PERL_SOURCE_SPECS,
    what = "perl_binary",
    allow_files = PERL_SOURCE_EXTS,
    upstream_providers = None,
    runtime = "besteffort",
    quality_tools = PERL_QUALITY_TOOLS,
)

_perl_forward_test = dx_executable_forward_rule(
    kind = "test",
    provides = _PERL_EXEC_PROVIDES,
    required_providers = [],
    quality_specs = PERL_SOURCE_SPECS,
    what = "perl_test",
    allow_files = PERL_SOURCE_EXTS,
    upstream_providers = None,
    extra_attrs = dx_lcov_merger_attr(),
    runtime = "besteffort",
    quality_tools = PERL_QUALITY_TOOLS,
)

PERL_DEFERRED_SKIP_TAGS = ["no-typecheck"]

def perl_quality_srcs(main, srcs):
    """Returns the hand-written sources a Perl target owns: main first, then srcs."""
    owned = [main] if main != None else []
    return owned + [s for s in (srcs or []) if s != main]

def perl_compatible_with(target_compatible_with):
    """Appends the rules_perl platform guard to the caller's constraints."""
    return (target_compatible_with or []) + PERL_COMPATIBLE_WITH

def perl_deferred_tags(tags):
    """Adds the skip tags that keep the deferred perl class out of quality aspects."""
    out = list(tags or [])
    for tag in PERL_DEFERRED_SKIP_TAGS:
        if tag not in out:
            out.append(tag)
    return out

def _perl_kwargs(kwargs):
    return dict(
        kwargs,
        tags = perl_deferred_tags(kwargs.get("tags")),
        target_compatible_with = perl_compatible_with(kwargs.get("target_compatible_with")),
    )

def perl_library(name, srcs = None, visibility = None, **kwargs):
    """Thin wrapper over perl_library."""
    kwargs = _perl_kwargs(kwargs)
    dx_wrap(name, _perl_library, _perl_library_forward, srcs or [], visibility = visibility, **kwargs)

def perl_binary(name, srcs = None, main = None, visibility = None, **kwargs):
    """Thin wrapper over perl_binary."""
    kwargs = _perl_kwargs(kwargs)
    upstream = dict(kwargs, main = main) if main != None else kwargs
    dx_wrap_binary(name, _perl_binary, _perl_binary_forward, srcs, visibility = visibility, upstream_kwargs = upstream, quality_srcs = perl_quality_srcs(main, srcs), **kwargs)

def perl_test(name, srcs = None, main = None, visibility = None, **kwargs):
    """Thin wrapper over perl_test."""
    kwargs = _perl_kwargs(kwargs)
    upstream = dict(kwargs, main = main) if main != None else kwargs
    dx_wrap_test(name, _perl_test, _perl_forward_test, srcs, visibility = visibility, upstream_kwargs = upstream, quality_srcs = perl_quality_srcs(main, srcs), coverage_collector = PERL_COVERAGE_COLLECTOR, **kwargs)
