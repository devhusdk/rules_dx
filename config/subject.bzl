"""Analysis subject exposing the canonical quality settings."""

load("//core:defs.bzl", "DxSubjectInfo")

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

def _exec_settings_subject_impl(ctx):
    return [
        DefaultInfo(files = depset([])),
        DxSubjectInfo(fields = {
            "exec_workspace": ctx.attr._probe[DxSubjectInfo].fields["workspace"],
        }),
    ]

exec_settings_subject = rule(
    implementation = _exec_settings_subject_impl,
    doc = "Analysis subject reporting the workspace policy seen in the exec configuration.",
    attrs = {
        "_probe": attr.label(cfg = "exec", default = "//config:settings_under_test"),
    },
)
