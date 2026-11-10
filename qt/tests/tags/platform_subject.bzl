"""Analysis subject that reports another subject's fields as built for one platform."""

load("//core:defs.bzl", "DxSubjectInfo")
load("//core:platform.bzl", "platform_transition")

def _platform_subject_impl(ctx):
    """Forwards the fields of the subject built for ctx.attr.platform."""
    return [
        DefaultInfo(files = depset([])),
        ctx.attr.subject[0][DxSubjectInfo],
    ]

platform_subject = rule(
    implementation = _platform_subject_impl,
    doc = "Reports the fields of a subject built for a platform.",
    attrs = {
        "platform": attr.label(
            doc = "Platform the subject is built for. Defaults to the current target platform.",
            providers = [platform_common.PlatformInfo],
        ),
        "subject": attr.label(
            cfg = platform_transition,
            doc = "Subject whose fields are reported.",
            mandatory = True,
            providers = [DxSubjectInfo],
        ),
    },
)
