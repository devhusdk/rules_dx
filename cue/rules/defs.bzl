"""CUE source ownership for dx quality."""

load("//libs/starlark:wrapper.bzl", "dx_quality_sources")

def _cue_library_impl(ctx):
    return [
        DefaultInfo(files = depset(
            ctx.files.srcs,
            transitive = [dep[DefaultInfo].files for dep in ctx.attr.deps],
        )),
        dx_quality_sources(ctx.files.srcs, [("cue", ["cue"])], str(ctx.label)),
    ]

cue_library = rule(
    implementation = _cue_library_impl,
    doc = "Owns the CUE files of one package for dx format.",
    attrs = {
        "srcs": attr.label_list(allow_files = [".cue"], doc = "CUE files of this package."),
        "deps": attr.label_list(providers = [DefaultInfo], doc = "CUE libraries this package imports."),
    },
)
