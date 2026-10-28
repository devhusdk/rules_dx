"""LaTeX source ownership for dx quality."""

load("//libs/starlark:wrapper.bzl", "dx_quality_sources")

def _latex_library_impl(ctx):
    return [
        DefaultInfo(files = depset(
            ctx.files.srcs,
            transitive = [dep[DefaultInfo].files for dep in ctx.attr.deps],
        )),
        dx_quality_sources(ctx.files.srcs, [("latex", ["tex", "sty", "cls"])], str(ctx.label)),
    ]

latex_library = rule(
    implementation = _latex_library_impl,
    doc = "Owns the LaTeX files of one package for dx quality.",
    attrs = {
        "srcs": attr.label_list(allow_files = [".tex", ".sty", ".cls"], doc = "LaTeX files of this package."),
        "deps": attr.label_list(providers = [DefaultInfo], doc = "LaTeX libraries this package depends on."),
    },
)
