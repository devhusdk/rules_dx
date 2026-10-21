"""Minimal Ruby wrappers."""

load("@rules_ruby//ruby:defs.bzl", _rb_binary = "rb_binary", _rb_library = "rb_library", _rb_test = "rb_test")
load("@rules_ruby//ruby/private:providers.bzl", "BundlerInfo", "RubyFilesInfo")
load("//libs/starlark:wrapper.bzl", "dx_executable_forward_rule", "dx_lcov_merger_attr", "dx_library_forward_rule", "dx_wrap", "dx_wrap_binary", "dx_wrap_test")
load("//quality:sources.bzl", "QualitySourcesInfo")

_DX_RUBY_LIBRARY_PROVIDES = [
    RubyFilesInfo,
    DefaultInfo,
    InstrumentedFilesInfo,
    QualitySourcesInfo,
]

_DX_RUBY_EXEC_PROVIDES = [
    DefaultInfo,
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
    provides = _DX_RUBY_EXEC_PROVIDES,
    required_providers = [],
    quality_specs = _DX_RUBY_SOURCE_SPECS,
    what = "ruby_*",
    allow_files = _DX_RUBY_SOURCE_EXTS,
    upstream_providers = [[RubyFilesInfo]],
    optional_providers = [RubyFilesInfo, BundlerInfo],
    runtime = "besteffort",
)

_ruby_forward_test = dx_executable_forward_rule(
    kind = "test",
    provides = _DX_RUBY_EXEC_PROVIDES,
    required_providers = [],
    quality_specs = _DX_RUBY_SOURCE_SPECS,
    what = "ruby_*",
    allow_files = _DX_RUBY_SOURCE_EXTS,
    upstream_providers = [[RubyFilesInfo]],
    extra_attrs = dx_lcov_merger_attr(),
    optional_providers = [RubyFilesInfo, BundlerInfo],
    runtime = "besteffort",
)

def ruby_effective_srcs(srcs):
    """Returns the effective direct sources for a ruby binary or test shape."""
    return srcs if srcs != None else []

def _ruby_wrap_library(name, srcs, visibility = None, **kwargs):
    dx_wrap(name, _rb_library, _ruby_library_forward, srcs, visibility = visibility, **kwargs)

def _ruby_wrap_binary(name, srcs, visibility = None, **kwargs):
    dx_wrap_binary(name, _rb_binary, _ruby_binary_forward, srcs, visibility = visibility, **kwargs)

def ruby_library(name, srcs, visibility = None, **kwargs):
    """Minimal wrapper over rb_library."""
    _ruby_wrap_library(name, srcs, visibility = visibility, **kwargs)

def ruby_binary(name, srcs = None, visibility = None, **kwargs):
    """Minimal wrapper over rb_binary."""
    effective_srcs = ruby_effective_srcs(srcs)
    if "srcs" in kwargs:
        kwargs.pop("srcs")
    _ruby_wrap_binary(name, effective_srcs, visibility = visibility, **kwargs)

def ruby_test(name, srcs, visibility = None, **kwargs):
    """Minimal wrapper over rb_test."""
    dx_wrap_test(name, _rb_test, _ruby_forward_test, srcs, visibility = visibility, **kwargs)
