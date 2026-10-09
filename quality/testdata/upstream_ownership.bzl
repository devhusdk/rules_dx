"""Ownership probes for ordinary upstream native targets."""

load("@rules_cc//cc/common:cc_common.bzl", "cc_common")
load("@rules_cc//cc/common:cc_info.bzl", "CcInfo")
load("//libs/starlark:defs.bzl", "DxSubjectInfo", "starlark_test")
load("//quality:real_aspects.bzl", "upstream_direct_sources")

def _render_ownership(direct_sources):
    """Renders derived ownership as sorted class to basename text."""
    parts = []
    for class_id in sorted(direct_sources.keys()):
        names = sorted([f.basename for f in direct_sources[class_id].to_list()])
        parts.append(class_id + ":" + ",".join(names))
    if len(parts) == 0:
        return "(empty)"
    return ";".join(parts)

def _upstream_ownership_probe_impl(target, ctx):
    """Reports the upstream ownership derivation for one target."""
    (recognized, direct_sources, error) = upstream_direct_sources(target, ctx)
    return [DxSubjectInfo(fields = {
        "error": error if error != "" else "(none)",
        "ownership": _render_ownership(direct_sources),
        "recognized": str(recognized),
    })]

upstream_ownership_probe = aspect(
    implementation = _upstream_ownership_probe_impl,
)

def _upstream_ownership_subject_impl(ctx):
    """Re-exposes the probe observations as subject fields."""
    info = ctx.attr.target[DxSubjectInfo]
    return [
        DefaultInfo(files = depset([])),
        DxSubjectInfo(fields = dict(info.fields)),
    ]

upstream_ownership_subject = rule(
    implementation = _upstream_ownership_subject_impl,
    attrs = {
        "target": attr.label(
            aspects = [upstream_ownership_probe],
            mandatory = True,
        ),
    },
)

def _no_context_cc_impl(_ctx):
    """Provides native compilation info without declared sources."""
    return [CcInfo(compilation_context = cc_common.create_compilation_context())]

no_context_cc = rule(
    implementation = _no_context_cc_impl,
    attrs = {},
)

UPSTREAM_OWNERSHIP_OBSERVATIONS = """subject //quality/testdata:upstream_cc_configured_subject
field error=(none)
field ownership=c:upstream_hello.h;cpp:upstream_hello.cc
field recognized=True
aspect_field aspect_seen=True
aspect_field field_count=3
aspect_field has_subject=True
aspect_field subject_label=//quality/testdata:upstream_cc_configured_subject
aspect_field transitive_count=0
subject //quality/testdata:upstream_cc_empty_subject
field error=(none)
field ownership=(empty)
field recognized=True
aspect_field aspect_seen=True
aspect_field field_count=3
aspect_field has_subject=True
aspect_field subject_label=//quality/testdata:upstream_cc_empty_subject
aspect_field transitive_count=0
subject //quality/testdata:upstream_cc_generated_subject
field error=(none)
field ownership=(empty)
field recognized=True
aspect_field aspect_seen=True
aspect_field field_count=3
aspect_field has_subject=True
aspect_field subject_label=//quality/testdata:upstream_cc_generated_subject
aspect_field transitive_count=0
subject //quality/testdata:upstream_cc_subject
field error=(none)
field ownership=c:upstream_hello.h;cpp:upstream_hello.cc
field recognized=True
aspect_field aspect_seen=True
aspect_field field_count=3
aspect_field has_subject=True
aspect_field subject_label=//quality/testdata:upstream_cc_subject
aspect_field transitive_count=0
subject //quality/testdata:upstream_rust_subject
field error=(none)
field ownership=rust:upstream_clean.rs
field recognized=True
aspect_field aspect_seen=True
aspect_field field_count=3
aspect_field has_subject=True
aspect_field subject_label=//quality/testdata:upstream_rust_subject
aspect_field transitive_count=0"""

def upstream_ownership_tests(name, subjects):
    """Instantiates the upstream ownership probe test."""
    starlark_test(
        name = name,
        mode = "analysis",
        subjects = subjects,
        expected_observations = UPSTREAM_OWNERSHIP_OBSERVATIONS,
    )
