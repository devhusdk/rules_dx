"""Subject that reports what platform_identity returns for a platform."""

load("//libs/starlark:defs.bzl", "DxSubjectInfo")
load("//libs/starlark:platform.bzl", "PLATFORM_ATTRS", "platform_identity")

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
