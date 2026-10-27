"""Minimal Ruby wrappers."""

load("@rules_ruby//ruby:defs.bzl", _rb_binary = "rb_binary", _rb_library = "rb_library", _rb_test = "rb_test")
load("@rules_ruby//ruby/private:providers.bzl", "BundlerInfo", "RubyFilesInfo")
load("//libs/starlark:wrapper.bzl", "DX_EXEC_PROVIDES", "dx_executable_forward_rule", "dx_lcov_merger_attr", "dx_library_forward_rule", "dx_wrap", "dx_wrap_binary", "dx_wrap_test")
load("//quality:sources.bzl", "QualitySourcesInfo")
load("//ruby/coverage:pins.bzl", "RUBY_COVERAGE_GEMS", "gem_dir")

_DX_RUBY_LIBRARY_PROVIDES = [
    RubyFilesInfo,
    DefaultInfo,
    InstrumentedFilesInfo,
    QualitySourcesInfo,
]

_DX_RUBY_SOURCE_SPECS = [("ruby", "rb")]
_DX_RUBY_SOURCE_EXTS = [".rb"]

_ruby_library_forward = dx_library_forward_rule(
    provides = _DX_RUBY_LIBRARY_PROVIDES,
    required_providers = [(RubyFilesInfo, "RubyFilesInfo")],
    quality_specs = _DX_RUBY_SOURCE_SPECS,
    what = "ruby_*",
    allow_files = _DX_RUBY_SOURCE_EXTS,
    upstream_providers = [[RubyFilesInfo]],
    runtime = "besteffort",
)

_ruby_binary_forward = dx_executable_forward_rule(
    kind = "executable",
    provides = DX_EXEC_PROVIDES,
    required_providers = [],
    quality_specs = _DX_RUBY_SOURCE_SPECS,
    what = "ruby_*",
    allow_files = _DX_RUBY_SOURCE_EXTS,
    upstream_providers = [[RubyFilesInfo]],
    optional_providers = [RubyFilesInfo, BundlerInfo],
    runtime = "besteffort",
)

def ruby_coverage_lib_dirs(short_paths):
    """Returns the runfiles-relative lib dir of each pinned SimpleCov gem, in pin order."""
    dirs = []
    for gem in RUBY_COVERAGE_GEMS:
        suffix = "/" + gem_dir(gem) + "/lib/" + gem.entry
        found = [p for p in short_paths if p.endswith(suffix)]
        if not found:
            fail("ruby coverage: no " + gem_dir(gem) + "/lib/" + gem.entry + " in the pinned gems")
        dirs.append(found[0][:-len("/" + gem.entry)])
    return dirs

def _ruby_test_coverage(ctx, upstream):
    """Puts the pinned SimpleCov gems on RUBYLIB so rules_ruby's coverage helper loads them (#3954)."""
    if InstrumentedFilesInfo in upstream:
        instrumented = upstream[InstrumentedFilesInfo]
    else:
        instrumented = coverage_common.instrumented_files_info(
            ctx,
            source_attributes = ["srcs"],
            dependency_attributes = ["upstream"],
        )
    gems = ctx.files._ruby_coverage_gems
    env = {}
    inherited = []
    if RunEnvironmentInfo in upstream:
        env.update(upstream[RunEnvironmentInfo].environment)
        inherited = upstream[RunEnvironmentInfo].inherited_environment
    sep = ";" if ctx.target_platform_has_constraint(ctx.attr._ruby_windows[platform_common.ConstraintValueInfo]) else ":"
    dirs = ruby_coverage_lib_dirs([f.short_path for f in gems])
    if env.get("RUBYLIB"):
        dirs.append(env["RUBYLIB"])
    env["RUBYLIB"] = sep.join(dirs)
    return struct(
        instrumented = instrumented,
        env = RunEnvironmentInfo(environment = env, inherited_environment = inherited),
        runfiles = ctx.runfiles(files = gems),
    )

_ruby_forward_test = dx_executable_forward_rule(
    kind = "test",
    provides = DX_EXEC_PROVIDES,
    required_providers = [],
    quality_specs = _DX_RUBY_SOURCE_SPECS,
    what = "ruby_*",
    allow_files = _DX_RUBY_SOURCE_EXTS,
    upstream_providers = [[RubyFilesInfo]],
    extra_attrs = dx_lcov_merger_attr() | {
        "_ruby_coverage_gems": attr.label(default = Label("//ruby/coverage:gems")),
        "_ruby_windows": attr.label(default = Label("@platforms//os:windows"), providers = [platform_common.ConstraintValueInfo]),
    },
    optional_providers = [RubyFilesInfo, BundlerInfo],
    runtime = "besteffort",
    coverage_providers = _ruby_test_coverage,
)

def ruby_library(name, srcs, visibility = None, **kwargs):
    """Minimal wrapper over rb_library; takes .rb srcs."""
    dx_wrap(name, _rb_library, _ruby_library_forward, srcs, visibility = visibility, **kwargs)

def ruby_binary(name, srcs = None, visibility = None, **kwargs):
    """Minimal wrapper over rb_binary; srcs is optional."""
    dx_wrap_binary(name, _rb_binary, _ruby_binary_forward, srcs, visibility = visibility, **kwargs)

def ruby_test(name, srcs, visibility = None, **kwargs):
    """Minimal wrapper over rb_test; set main to a runner such as an RSpec bin."""
    dx_wrap_test(name, _rb_test, _ruby_forward_test, srcs, visibility = visibility, **kwargs)
