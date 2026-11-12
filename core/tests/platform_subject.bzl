"""Subject that reports what platform_identity returns for a platform."""

load("//core:defs.bzl", "DxSubjectInfo")
load("//core:platform.bzl", "DxPlatformInfo", "PLATFORM_ATTRS", "platform_identity", "platforms_transition")

def _platform_subject_impl(ctx):
    """Reports the os and cpu seen for one platform."""
    id = platform_identity(ctx)
    return [
        DefaultInfo(files = depset([])),
        DxSubjectInfo(fields = {"cpu": id.cpu, "os": id.os}),
    ]

platform_subject = rule(
    implementation = _platform_subject_impl,
    doc = "Reports the os and cpu platform_identity returns.",
    attrs = PLATFORM_ATTRS,
)

def _platforms_subject_impl(ctx):
    """Reports the os and cpu of each configuration the probe was built in."""
    ids = sorted([probe[DxPlatformInfo].os + "_" + probe[DxPlatformInfo].cpu for probe in ctx.attr.probe])
    return [
        DefaultInfo(files = depset([])),
        DxSubjectInfo(fields = {"configurations": ",".join(ids)}),
    ]

platforms_subject = rule(
    implementation = _platforms_subject_impl,
    doc = "Reports the os and cpu of each configuration platforms_transition builds the probe in.",
    attrs = {
        "platforms": attr.string_list(doc = "Platform labels."),
        "probe": attr.label(cfg = platforms_transition(), default = Label("//core:platform_probe")),
    },
)
