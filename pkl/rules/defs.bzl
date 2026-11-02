"""Pkl source ownership for dx quality."""

load("//libs/starlark:wrapper.bzl", "dx_source_library")

pkl_library = dx_source_library("pkl", ["pkl"], "Pkl")
