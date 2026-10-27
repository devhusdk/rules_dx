"""XML source ownership for dx quality."""

load("//libs/starlark:wrapper.bzl", "dx_quality_sources")

_EXTENSIONS = ["xml", "xsd", "xsl", "xslt", "wsdl"]

def _xml_library_impl(ctx):
    return [
        DefaultInfo(files = depset(
            ctx.files.srcs,
            transitive = [dep[DefaultInfo].files for dep in ctx.attr.deps],
        )),
        dx_quality_sources(ctx.files.srcs, [("xml", _EXTENSIONS)], str(ctx.label)),
    ]

xml_library = rule(
    implementation = _xml_library_impl,
    doc = "Owns the XML files of one package for dx quality.",
    attrs = {
        "srcs": attr.label_list(allow_files = ["." + ext for ext in _EXTENSIONS], doc = "XML files of this package."),
        "deps": attr.label_list(providers = [DefaultInfo], doc = "XML libraries this package depends on."),
    },
)
