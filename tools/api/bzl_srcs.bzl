"""Narrows a set of source files to the Starlark files among them."""

visibility("private")

def _bzl_srcs_impl(ctx):
    files = [
        file
        for file in ctx.files.srcs
        if file.extension in ("bzl", "scl")
    ]
    return [DefaultInfo(files = depset(files))]

bzl_srcs = rule(
    implementation = _bzl_srcs_impl,
    doc = "Passes on only the `.bzl` and `.scl` files of `srcs`, for `starlark_doc_extract` deps.",
    attrs = {
        "srcs": attr.label_list(
            allow_files = True,
            doc = "Targets whose files include the Starlark sources.",
        ),
    },
)
