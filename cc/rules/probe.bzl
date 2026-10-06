"""Conformance subjects for the minimal C/C++ wrappers."""

load("//libs/starlark:canonical.bzl", "strip_canonical")
load("//libs/starlark:defs.bzl", "DxAspectInfo", "DxSubjectInfo")
load("//quality:sources.bzl", "QualitySourcesInfo")

DX_CC_DECLARED_ATTRS = [
    "compatible_with",
    "env",
    "env_inherit",
    "exec_compatible_with",
    "flaky",
    "hdrs",
    "shard_count",
    "size",
    "tags",
    "target_compatible_with",
    "testonly",
    "timeout",
]

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

def _render_item(item):
    if type(item) == "Target":
        return item.label.name
    return str(item)

def _render_declared(value):
    if value == None:
        return "(unset)"
    if type(value) == "dict":
        return ",".join([key + "=" + str(value[key]) for key in sorted(value.keys())])
    if type(value) == "list" or type(value) == "tuple":
        return ",".join([_render_item(item) for item in value])
    return _render_item(value)

def _render_declared_fields(fields):
    return ",".join([name + "=" + fields[name] for name in DX_CC_DECLARED_ATTRS])

def _dx_cc_declared_attrs_impl(target, ctx):
    """Renders the attributes the visited public target declares."""
    fields = {"subject_label": _label_text(target.label)}
    for name in DX_CC_DECLARED_ATTRS:
        fields[name] = _render_declared(getattr(ctx.rule.attr, name, None))
    return [DxAspectInfo(fields = fields)]

dx_cc_declared_attrs = aspect(
    implementation = _dx_cc_declared_attrs_impl,
    attr_aspects = [],
)

def _cc_wrapper_subject_impl(ctx):
    wrapper = ctx.attr.wrapper
    upstream = ctx.attr.upstream
    fields = {
        "direct_sources": _render_direct_sources(wrapper[QualitySourcesInfo]),
        "upstream": _label_text(ctx.attr.upstream.label),
        "upstream_declared": _render_declared_fields(upstream[DxAspectInfo].fields),
        "upstream_has_instrumented_files": str(InstrumentedFilesInfo in upstream),
        "upstream_has_quality_sources": str(QualitySourcesInfo in upstream),
        "wrapper": _label_text(ctx.attr.wrapper.label),
        "wrapper_declared": _render_declared_fields(wrapper[DxAspectInfo].fields),
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
        "upstream": attr.label(
            aspects = [dx_cc_declared_attrs],
            mandatory = True,
        ),
        "wrapper": attr.label(
            aspects = [dx_cc_declared_attrs],
            mandatory = True,
        ),
    },
)
