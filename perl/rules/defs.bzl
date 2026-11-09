"""Thin Perl wrappers over rules_perl."""

load("@rules_perl//perl:defs.bzl", _PerlInfo = "PerlInfo", _perl_binary = "perl_binary", _perl_library = "perl_library", _perl_test = "perl_test")
load("//libs/starlark:cells.bzl", "unsupported_constraint")
load("//libs/starlark:entry.bzl", "dx_entry_quality_srcs", "dx_infer_main", "dx_main_candidates")
load("//libs/starlark:wrapper.bzl", "DX_EXEC_PROVIDES", "dx_executable_forward_rule", "dx_language_kwargs", "dx_lcov_merger_attr", "dx_library_forward_rule", "dx_wrap", "dx_wrap_binary", "dx_wrap_test")
load("//quality:sources.bzl", "QualitySourcesInfo", "QualityToolsInfo")

PerlInfo = _PerlInfo

PERL_QUALITY_TOOLS = {
    "perlcritic": Label("//perl/tools:perlcritic"),
    "perltidy": Label("//perl/tools:perltidy"),
}

_PERL_EXEC_PROVIDES = DX_EXEC_PROVIDES + [QualityToolsInfo]

PERL_SOURCE_SPECS = [("perl", ["pl", "pm", "t"])]

PERL_SOURCE_EXTS = [".pl", ".pm", ".t"]

PERL_COMPATIBLE_WITH = select({
    Label("//perl/rules:rules_perl_platform"): [],
    "//conditions:default": [Label("@platforms//:incompatible"), unsupported_constraint("//perl/rules")],
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
    instrument_srcs = True,
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

def _perl_test_coverage(ctx, upstream):
    """Loads the DxLcov collector in every perl the test starts (#7324)."""
    instrumented = coverage_common.instrumented_files_info(
        ctx,
        source_attributes = ["srcs"],
        dependency_attributes = ["upstream", "deps"],
    )
    collector = ctx.files._perl_coverage
    env = {}
    inherited = []
    if RunEnvironmentInfo in upstream:
        env.update(upstream[RunEnvironmentInfo].environment)
        inherited = upstream[RunEnvironmentInfo].inherited_environment
    sep = ";" if ctx.target_platform_has_constraint(ctx.attr._perl_windows[platform_common.ConstraintValueInfo]) else ":"
    dirs = [collector[0].short_path.rpartition("/")[0]]
    if env.get("PERL5LIB"):
        dirs.append(env["PERL5LIB"])
    env["PERL5LIB"] = sep.join(dirs)
    env["PERL5OPT"] = " ".join(["-MDxLcov"] + ([env["PERL5OPT"]] if env.get("PERL5OPT") else []))
    return struct(
        instrumented = instrumented,
        env = RunEnvironmentInfo(environment = env, inherited_environment = inherited),
        runfiles = ctx.runfiles(files = collector),
    )

_perl_forward_test = dx_executable_forward_rule(
    kind = "test",
    provides = _PERL_EXEC_PROVIDES,
    required_providers = [],
    quality_specs = PERL_SOURCE_SPECS,
    what = "perl_test",
    allow_files = PERL_SOURCE_EXTS,
    upstream_providers = None,
    extra_attrs = dx_lcov_merger_attr() | {
        "deps": attr.label_list(doc = "Perl libraries the target depends on.", providers = [_PerlInfo]),
        "_perl_coverage": attr.label(default = Label("//perl/coverage:collector")),
        "_perl_windows": attr.label(default = Label("@platforms//os:windows"), providers = [platform_common.ConstraintValueInfo]),
    },
    runtime = "besteffort",
    quality_tools = PERL_QUALITY_TOOLS,
    coverage_providers = _perl_test_coverage,
)

PERL_DEFERRED_SKIP_TAGS = ["no-typecheck"]

def perl_library(name, srcs = None, visibility = None, **kwargs):
    """Thin wrapper over perl_library."""
    kwargs = dx_language_kwargs(kwargs, PERL_DEFERRED_SKIP_TAGS, PERL_COMPATIBLE_WITH)
    dx_wrap(name, _perl_library, _perl_library_forward, srcs or [], visibility = visibility, **kwargs)

def perl_binary(name, *, srcs = None, main = None, visibility = None, **kwargs):
    """Thin wrapper over perl_binary."""
    main = dx_infer_main("perl_binary", name, main, srcs, dx_main_candidates(name, ".pl"))
    kwargs = dx_language_kwargs(kwargs, PERL_DEFERRED_SKIP_TAGS, PERL_COMPATIBLE_WITH)
    upstream = dict(kwargs, main = main)
    dx_wrap_binary(name, _perl_binary, _perl_binary_forward, srcs, visibility = visibility, upstream_kwargs = upstream, quality_srcs = dx_entry_quality_srcs(main, srcs), **kwargs)

def perl_test(name, *, srcs = None, main = None, visibility = None, **kwargs):
    """Thin wrapper over perl_test."""
    main = dx_infer_main("perl_test", name, main, srcs, dx_main_candidates(name, ".pl"))
    kwargs = dx_language_kwargs(kwargs, PERL_DEFERRED_SKIP_TAGS, PERL_COMPATIBLE_WITH)
    upstream = dict(kwargs, main = main)
    dx_wrap_test(name, _perl_test, _perl_forward_test, srcs, visibility = visibility, upstream_kwargs = upstream, extra_forward_kwargs = {"deps": kwargs.get("deps", [])}, quality_srcs = dx_entry_quality_srcs(main, srcs), **kwargs)
