"""HTML template source ownership for dx quality."""

load("//libs/starlark:wrapper.bzl", "dx_quality_sources")

_EXTENSIONS = ["html", "htm", "jinja", "jinja2", "j2", "njk", "hbs", "gohtml", "tmpl"]

def _html_template_library_impl(ctx):
    return [
        DefaultInfo(files = depset(
            ctx.files.srcs,
            transitive = [dep[DefaultInfo].files for dep in ctx.attr.deps],
        )),
        dx_quality_sources(ctx.files.srcs, [("html_template", _EXTENSIONS)], str(ctx.label)),
    ]

html_template_library = rule(
    implementation = _html_template_library_impl,
    doc = "Owns the HTML template files of one package for dx quality.",
    attrs = {
        "srcs": attr.label_list(allow_files = ["." + ext for ext in _EXTENSIONS], doc = "Template files of this package."),
        "deps": attr.label_list(providers = [DefaultInfo], doc = "Template libraries this package includes from."),
    },
)
