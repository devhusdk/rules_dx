"""Typst source ownership for dx quality."""

load("//libs/starlark:wrapper.bzl", "dx_source_library")

typst_library = dx_source_library("typst", ["typ"], "Typst")
