"""Public Julia module extensions."""

load("@rules_julia//julia:extensions.bzl", _julia_toolchain = "julia")
load("@rules_julia//julia/pkg:extensions.bzl", _pkg = "pkg")

# The packages extension carries the `julia` ecosystem id from julia/lang.json, the name
# `dx adopt` and `dx update` look it up by.
julia = _pkg

julia_toolchain = _julia_toolchain
