"""Analysis subject exposing the canonical quality settings."""

load("//libs/starlark:defs.bzl", "DxSubjectInfo")

def _settings_subject_impl(ctx):
    return [
        DefaultInfo(files = depset([])),
        DxSubjectInfo(fields = {
            "workspace": str(ctx.attr._workspace.label),
        }),
    ]

settings_subject = rule(
    implementation = _settings_subject_impl,
    doc = "Analysis subject reporting the canonical quality settings.",
    attrs = {
        "_workspace": attr.label(default = "//config:workspace"),
    },
)
