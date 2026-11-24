"""Public Julia module extensions."""

load("@rules_julia//julia:extensions.bzl", _julia_toolchain = "julia")
load("@rules_julia//julia/pkg:extensions.bzl", _pkg = "pkg")

julia = _pkg

julia_toolchain = _julia_toolchain
