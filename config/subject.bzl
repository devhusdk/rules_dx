"""Analysis subject exposing the canonical quality settings."""

load("@bazel_skylib//rules:common_settings.bzl", "BuildSettingInfo")
load("//libs/starlark:defs.bzl", "DxSubjectInfo")

def _settings_subject_impl(ctx):
    return [
        DefaultInfo(files = depset([])),
        DxSubjectInfo(fields = {
            "spotbugs_effort": ctx.attr._spotbugs_effort[BuildSettingInfo].value,
            "spotbugs_threshold": ctx.attr._spotbugs_threshold[BuildSettingInfo].value,
            "workspace": str(ctx.attr._workspace.label),
        }),
    ]

settings_subject = rule(
    implementation = _settings_subject_impl,
    doc = "Analysis subject reporting the canonical quality settings.",
    attrs = {
        "_spotbugs_effort": attr.label(default = "//config:spotbugs_effort"),
        "_spotbugs_threshold": attr.label(default = "//config:spotbugs_threshold"),
        "_workspace": attr.label(default = "//config:workspace"),
    },
)
