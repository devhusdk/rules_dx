"""Gherkin source ownership for dx quality."""

load("//libs/starlark:wrapper.bzl", "dx_quality_sources")

_EXTENSIONS = ["feature"]

def _gherkin_library_impl(ctx):
    return [
        DefaultInfo(files = depset(
            ctx.files.srcs,
            transitive = [dep[DefaultInfo].files for dep in ctx.attr.deps],
        )),
        dx_quality_sources(ctx.files.srcs, [("gherkin", _EXTENSIONS)], str(ctx.label)),
    ]

gherkin_library = rule(
    implementation = _gherkin_library_impl,
    doc = "Owns the Gherkin files of one package for dx quality.",
    attrs = {
        "srcs": attr.label_list(allow_files = ["." + ext for ext in _EXTENSIONS], doc = "Gherkin files of this package."),
        "deps": attr.label_list(providers = [DefaultInfo], doc = "Gherkin libraries this package depends on."),
    },
)
