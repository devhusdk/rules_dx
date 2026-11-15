"""Public Julia module extensions."""

load("@rules_julia//julia:extensions.bzl", _toolchain = "julia")
load("@rules_julia//julia/pkg:extensions.bzl", _pkg = "pkg")

julia = _pkg

toolchain = _toolchain
