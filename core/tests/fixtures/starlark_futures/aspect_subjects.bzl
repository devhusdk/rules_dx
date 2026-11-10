"""Concrete aspect-subjects use case."""

load("//core:defs.bzl", "DxSubjectInfo")

def _aspect_leaf_impl(ctx):
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

aspect_leaf = rule(
    implementation = _aspect_leaf_impl,
    attrs = {
        "left": attr.int(default = 0),
        "right": attr.int(default = 0),
        "deps": attr.label_list(default = []),
    },
)

def _aspect_group_impl(ctx):
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.write(out, "note=" + ctx.attr.note + "\n")
    return [
        DefaultInfo(files = depset([out])),
        DxSubjectInfo(fields = {
            "dep_count": str(len(ctx.attr.deps)),
            "note": ctx.attr.note,
        }),
    ]

aspect_group = rule(
    implementation = _aspect_group_impl,
    attrs = {
        "deps": attr.label_list(default = []),
        "note": attr.string(default = ""),
    },
)
