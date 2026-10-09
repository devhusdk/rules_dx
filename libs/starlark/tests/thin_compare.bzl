"""Thin-candidate comparison subjects for the wrapper keep review."""

load("//libs/starlark:canonical.bzl", "strip_canonical")
load("//libs/starlark:defs.bzl", "DxSubjectInfo", "starlark_test")
load("//libs/starlark:wrapper.bzl", "dx_executable_forward_rule", "dx_library_forward_rule")
load("//quality:sources.bzl", "QualitySourcesInfo")

def _render_direct_sources(info):
    """Renders direct sources as sorted class to basename text."""
    if info == None:
        return "(none)"
    parts = []
    for class_id in sorted(info.direct_sources.keys()):
        names = sorted([f.basename for f in info.direct_sources[class_id].to_list()])
        parts.append(class_id + ":" + ",".join(names))
    if len(parts) == 0:
        return "(empty)"
    return ";".join(parts)

def _thin_compare_impl(ctx):
    """Compares one wrapper forwarder against its raw upstream target."""
    wrapper = ctx.attr.wrapper
    thin = ctx.attr.thin
    wrapper_quality = wrapper[QualitySourcesInfo] if QualitySourcesInfo in wrapper else None
    thin_quality = thin[QualitySourcesInfo] if QualitySourcesInfo in thin else None
    fields = {
        "kind": ctx.attr.kind,
        "labels_differ": str(ctx.attr.wrapper.label != ctx.attr.thin.label),
        "quality_only_on_wrapper": str(wrapper_quality != None and thin_quality == None),
        "thin_direct_sources": _render_direct_sources(thin_quality),
        "thin_has_instrumented_files": str(InstrumentedFilesInfo in thin),
        "thin_has_quality_sources": str(thin_quality != None),
        "thin_label": strip_canonical(str(ctx.attr.thin.label)),
        "wrapper_direct_sources": _render_direct_sources(wrapper_quality),
        "wrapper_has_instrumented_files": str(InstrumentedFilesInfo in wrapper),
        "wrapper_has_quality_sources": str(wrapper_quality != None),
        "wrapper_label": strip_canonical(str(ctx.attr.wrapper.label)),
    }
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.write(out, "\n".join([k + "=" + fields[k] for k in sorted(fields.keys())]) + "\n")
    return [
        DefaultInfo(files = depset([out])),
        DxSubjectInfo(fields = fields),
    ]

thin_compare_subject = rule(
    implementation = _thin_compare_impl,
    attrs = {
        "kind": attr.string(mandatory = True),
        "thin": attr.label(mandatory = True),
        "wrapper": attr.label(mandatory = True),
    },
)

thin_demo_library_forward = dx_library_forward_rule(
    provides = [DefaultInfo, QualitySourcesInfo],
    required_providers = [(QualitySourcesInfo, "QualitySourcesInfo")],
    quality_specs = [("text", ["txt"])],
    what = "thin_demo",
    allow_files = [".txt"],
    upstream_providers = None,
)

thin_demo_executable_forward = dx_executable_forward_rule(
    kind = "executable",
    provides = [DefaultInfo, QualitySourcesInfo],
    required_providers = [],
    quality_specs = [("text", ["txt"])],
    what = "thin_demo",
    allow_files = [".txt"],
    upstream_providers = None,
)

def thin_compare_cc_tests(name):
    """Instantiates the C++ wrapper against raw upstream comparison."""
    starlark_test(
        name = name,
        mode = "analysis",
        subjects = [":thin_cc_subject"],
        expected_observations = THIN_CC_OBSERVATIONS,
    )

def thin_compare_rust_tests(name):
    """Instantiates the Rust wrapper against raw upstream comparison."""
    starlark_test(
        name = name,
        mode = "analysis",
        subjects = [":thin_rust_subject"],
        expected_observations = THIN_RUST_OBSERVATIONS,
    )

THIN_CC_OBSERVATIONS = """subject //cc/tests/fixtures/hello:thin_cc_subject
file thin_cc_subject.txt
field kind=cc_library
field labels_differ=True
field quality_only_on_wrapper=True
field thin_direct_sources=(none)
field thin_has_instrumented_files=True
field thin_has_quality_sources=False
field thin_label=//cc/tests/fixtures/hello:hello_lib_upstream
field wrapper_direct_sources=c:hello.h;cpp:hello.cc
field wrapper_has_instrumented_files=True
field wrapper_has_quality_sources=True
field wrapper_label=//cc/tests/fixtures/hello:hello_lib
aspect_field aspect_seen=True
aspect_field field_count=11
aspect_field has_subject=True
aspect_field subject_label=//cc/tests/fixtures/hello:thin_cc_subject
aspect_field transitive_count=0"""

THIN_RUST_OBSERVATIONS = """subject //rust/tests/fixtures/hello:thin_rust_subject
file thin_rust_subject.txt
field kind=rust_library
field labels_differ=True
field quality_only_on_wrapper=True
field thin_direct_sources=(none)
field thin_has_instrumented_files=True
field thin_has_quality_sources=False
field thin_label=//rust/tests/fixtures/hello:hello_lib_upstream
field wrapper_direct_sources=rust:lib.rs
field wrapper_has_instrumented_files=True
field wrapper_has_quality_sources=True
field wrapper_label=//rust/tests/fixtures/hello:hello_lib
aspect_field aspect_seen=True
aspect_field field_count=11
aspect_field has_subject=True
aspect_field subject_label=//rust/tests/fixtures/hello:thin_rust_subject
aspect_field transitive_count=0"""
