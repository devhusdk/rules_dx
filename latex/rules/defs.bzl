"""LaTeX source ownership for dx quality."""

load("//libs/starlark:wrapper.bzl", "dx_source_library")

latex_library = dx_source_library("latex", ["tex", "sty", "cls"], "LaTeX")
