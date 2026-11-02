"""YAML source ownership for dx quality."""

load("//libs/starlark:wrapper.bzl", "dx_source_library")

yaml_library = dx_source_library("yaml", ["yaml", "yml"], "YAML")
