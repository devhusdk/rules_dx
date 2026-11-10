"""Analysis subject rule exposing DxSubjectInfo and one output file."""

load("//core:defs.bzl", "DxSubjectInfo")

def _example_subject_impl(ctx):
    total = ctx.attr.left + ctx.attr.right
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.write(out, "sum=" + str(total) + "\n")
    return [
        DefaultInfo(files = depset([out])),
        DxSubjectInfo(fields = {
            "left": str(ctx.attr.left),
            "right": str(ctx.attr.right),
            "sum": str(total),
        }),
    ]

example_subject = rule(
    implementation = _example_subject_impl,
    attrs = {
        "left": attr.int(default = 0),
        "right": attr.int(default = 0),
    },
)
