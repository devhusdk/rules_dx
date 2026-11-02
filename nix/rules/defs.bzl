"""Nix source ownership for dx quality."""

load("//libs/starlark:wrapper.bzl", "dx_source_library")

nix_library = dx_source_library("nix", ["nix"], "Nix")
