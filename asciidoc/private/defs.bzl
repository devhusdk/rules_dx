"""AsciiDoc source ownership for dx quality."""

load("//core:wrapper.bzl", "dx_source_library")

asciidoc_library = dx_source_library(
    "asciidoc",
    ["adoc", "asciidoc", "asc"],
    "AsciiDoc",
    tools = {"asciidoctor": Label("//javascript/private/quality/bin:asciidoctor")},
)
