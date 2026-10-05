"""Conformance subject for the minimal C/C++ wrappers."""

load("//libs/starlark:canonical.bzl", "strip_canonical")
load("//libs/starlark:defs.bzl", "DxSubjectInfo")
load("//quality:sources.bzl", "QualitySourcesInfo")

def _label_text(label):
    return strip_canonical(str(label))

def _sorted_basenames(files):
    return sorted([f.basename for f in files])

def _render_direct_sources(info):
    parts = []
    for class_id in sorted(info.direct_sources.keys()):
        parts.append(class_id + ":" + ",".join(_sorted_basenames(info.direct_sources[class_id].to_list())))
    if len(parts) == 0:
        return "(none)"
    return ";".join(parts)

def _cc_wrapper_subject_impl(ctx):
    wrapper = ctx.attr.wrapper
    upstream = ctx.attr.upstream
    fields = {
        "direct_sources": _render_direct_sources(wrapper[QualitySourcesInfo]),
        "upstream": _label_text(ctx.attr.upstream.label),
        "upstream_has_instrumented_files": str(InstrumentedFilesInfo in upstream),
        "upstream_has_quality_sources": str(QualitySourcesInfo in upstream),
        "wrapper": _label_text(ctx.attr.wrapper.label),
        "wrapper_has_instrumented_files": str(InstrumentedFilesInfo in wrapper),
        "wrapper_has_quality_sources": str(QualitySourcesInfo in wrapper),
        "wrapper_runfiles": str(len(wrapper[DefaultInfo].default_runfiles.files.to_list()) > 0),
    }
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.write(out, "\n".join([k + "=" + fields[k] for k in sorted(fields.keys())]) + "\n")
    return [
        DefaultInfo(files = depset([out])),
        DxSubjectInfo(fields = fields),
    ]

dx_cc_wrapper_subject = rule(
    implementation = _cc_wrapper_subject_impl,
    attrs = {
        "upstream": attr.label(mandatory = True),
        "wrapper": attr.label(mandatory = True),
    },
)
