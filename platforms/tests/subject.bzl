"""Subjects that report the platform flags and platform_select values after a platform transition."""

load("@bazel_skylib//rules:common_settings.bzl", "BuildSettingInfo")
load("//core/private/testing:providers.bzl", "DxSubjectInfo")
load("//platforms:defs.bzl", "DxTargetInfo", "platform_transition")

def _moved_subject_impl(ctx):
    target = ctx.attr._target[DxTargetInfo]
    return [
        DefaultInfo(files = depset([])),
        DxSubjectInfo(fields = {
            "current": ctx.attr._current[BuildSettingInfo].value,
            "min_os_android": ctx.attr._min_os_android[BuildSettingInfo].value,
            "runner": str(target.runner),
            "value": ctx.attr.value,
        }),
    ]

moved_subject = rule(
    implementation = _moved_subject_impl,
    doc = "Reports the platform flags and its value attr in the configuration platform_transition picks.",
    cfg = platform_transition,
    attrs = {
        "min_os": attr.string_dict(doc = "Minimum OS versions."),
        "platforms": attr.string_list(doc = "Platform names."),
        "value": attr.string(doc = "A platform_select value."),
        "_current": attr.label(default = "//platforms:current"),
        "_min_os_android": attr.label(default = "//platforms:min_os_android"),
        "_target": attr.label(default = "//platforms:current_target"),
    },
)

def _on_platform_impl(_settings, attr):
    return {"//command_line_option:platforms": [attr.platform]}

_on_platform = transition(
    implementation = _on_platform_impl,
    inputs = [],
    outputs = ["//command_line_option:platforms"],
)

def _platform_subject_impl(ctx):
    return [DefaultInfo(files = depset([])), DxSubjectInfo(fields = {"value": ctx.attr.value})]

platform_subject = rule(
    implementation = _platform_subject_impl,
    doc = "Reports its value attr, a platform_select, on the platform its platform attr names.",
    cfg = _on_platform,
    attrs = {
        "platform": attr.string(mandatory = True, doc = "Label of the platform to build on."),
        "value": attr.string(doc = "A platform_select value."),
    },
)
