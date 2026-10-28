"""AsciiDoc source ownership for dx quality."""

load("//libs/starlark:wrapper.bzl", "dx_quality_sources")
load("//quality:sources.bzl", "QualityToolsInfo")

def _asciidoc_library_impl(ctx):
    return [
        DefaultInfo(files = depset(
            ctx.files.srcs,
            transitive = [dep[DefaultInfo].files for dep in ctx.attr.deps],
        )),
        dx_quality_sources(ctx.files.srcs, [("asciidoc", ["adoc", "asciidoc", "asc"])], str(ctx.label)),
        # Vale converts AsciiDoc through asciidoctor, which only the owner loads.
        QualityToolsInfo(tools = {"asciidoctor": ctx.attr._asciidoctor[DefaultInfo].files_to_run}),
    ]

asciidoc_library = rule(
    implementation = _asciidoc_library_impl,
    doc = "Owns the AsciiDoc files of one package for dx quality.",
    attrs = {
        "srcs": attr.label_list(allow_files = [".adoc", ".asciidoc", ".asc"], doc = "AsciiDoc files of this package."),
        "deps": attr.label_list(providers = [DefaultInfo], doc = "AsciiDoc libraries this package depends on."),
        "_asciidoctor": attr.label(
            default = Label("//quality/tools/javascript/bin:asciidoctor"),
            cfg = "exec",
            executable = True,
        ),
    },
    provides = [DefaultInfo, QualityToolsInfo],
)
