"""Analysis subject observing upstream source ownership."""

load("//libs/starlark:canonical.bzl", "strip_canonical")
load("//libs/starlark:defs.bzl", "DxSubjectInfo")
load("//quality:sources.bzl", "QualitySourcesInfo")
load("//quality:upstream_ownership.bzl", "upstream_owned_sources")

def _render_owned(direct_sources):
    """Renders owned sources as class to basename text."""
    parts = []
    for class_id in sorted(direct_sources.keys()):
        names = sorted([f.basename for f in direct_sources[class_id].to_list()])
        parts.append(class_id + ":" + ",".join(names))
    if len(parts) == 0:
        return "(empty)"
    return ";".join(parts)

def _upstream_ownership_aspect_impl(target, ctx):
    if QualitySourcesInfo in target:
        owned = _render_owned(target[QualitySourcesInfo].direct_sources)
        status = "wrapper"
    else:
        info = upstream_owned_sources(target, ctx)
        if info == None:
            owned = "(none)"
            status = "unsupported"
        else:
            owned = _render_owned(info.direct_sources)
            status = "upstream"
    return [DxSubjectInfo(fields = {
        "label": strip_canonical(str(target.label)),
        "owned": owned,
        "status": status,
    })]

upstream_ownership_aspect = aspect(
    implementation = _upstream_ownership_aspect_impl,
    attr_aspects = [],
)

def _upstream_ownership_subject_impl(ctx):
    info = ctx.attr.target[DxSubjectInfo]
    return [
        DefaultInfo(files = depset([])),
        DxSubjectInfo(fields = dict(info.fields)),
    ]

upstream_ownership_subject = rule(
    implementation = _upstream_ownership_subject_impl,
    attrs = {
        "target": attr.label(
            aspects = [upstream_ownership_aspect],
            mandatory = True,
        ),
    },
)
