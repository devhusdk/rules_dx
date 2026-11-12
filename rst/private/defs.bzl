"""reStructuredText source ownership for dx quality."""

load("//core:wrapper.bzl", "dx_source_library")

rst_library = dx_source_library(
    "rst",
    ["rst", "rest"],
    "reStructuredText",
    tools = {"rst2html": Label("//core/quality/tools/python:rst2html")},
)
