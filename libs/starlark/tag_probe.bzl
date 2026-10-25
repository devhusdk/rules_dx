"""Analysis probe that reports the tags of each listed target."""

load("//libs/starlark:defs.bzl", "DxSubjectInfo")

_TagsInfo = provider(
    doc = "Tags of one probed target.",
    fields = {"tags": "Tag list of the target."},
)

def _tag_probe_impl(_target, ctx):
    """Reads the tags of one target."""
    return [_TagsInfo(tags = getattr(ctx.rule.attr, "tags", []))]

tag_probe = aspect(
    implementation = _tag_probe_impl,
    doc = "Reads the tags of each target it visits.",
)

def _tag_subject_impl(ctx):
    """Reports each target's tags as one subject field."""
    fields = {}
    for target in ctx.attr.targets:
        fields[target.label.name] = ",".join(target[_TagsInfo].tags) or "(none)"
    return [
        DefaultInfo(files = depset([])),
        DxSubjectInfo(fields = fields),
    ]

tag_subject = rule(
    implementation = _tag_subject_impl,
    doc = "Reports the tags of each listed target.",
    attrs = {
        "targets": attr.label_list(
            aspects = [tag_probe],
            doc = "Targets whose tags are reported.",
        ),
    },
)
