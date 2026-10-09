"""Upstream source ownership for ordinary non-wrapped targets."""

load("@rules_cc//cc/common:cc_info.bzl", "CcInfo")
load("@rules_rust//rust:defs.bzl", _rust_common = "rust_common")
load("//cc/rules:defs.bzl", "DX_CC_SOURCE_SPECS")
load("//libs/starlark:wrapper.bzl", "dx_quality_sources")
load("//quality:sources.bzl", "QualitySourcesInfo")
load("//rust/rules:defs.bzl", "DX_RUST_SOURCE_SPECS")

def _attr_files(rule_attr, name):
    """Collects the files named by one label-list rule attribute."""
    files = []
    for entry in getattr(rule_attr, name, []):
        files.extend(entry.files.to_list())
    return files

def upstream_owned_sources(target, ctx):
    """Derives QualitySourcesInfo from declared sources of an ordinary upstream target, or None when unsupported."""
    if QualitySourcesInfo in target:
        return None
    if _rust_common.crate_info in target or _rust_common.test_crate_info in target:
        return dx_quality_sources(
            _attr_files(ctx.rule.attr, "srcs"),
            DX_RUST_SOURCE_SPECS,
            str(target.label),
        )
    if CcInfo in target:
        return dx_quality_sources(
            _attr_files(ctx.rule.attr, "srcs") + _attr_files(ctx.rule.attr, "hdrs"),
            DX_CC_SOURCE_SPECS,
            str(target.label),
        )
    return None
