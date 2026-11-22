"""Subject that reports the platform flags and a platform_select value after platform_transition."""

load("@bazel_skylib//rules:common_settings.bzl", "BuildSettingInfo")
load("//core/private/testing:defs.bzl", "DxSubjectInfo")
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
