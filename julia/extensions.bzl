"""Public Julia module extensions."""

load("@rules_julia//julia:extensions.bzl", _julia_toolchain = "julia")
load("@rules_julia//julia/pkg:extensions.bzl", _pkg = "pkg")
load("//julia/private/runtime:hub.bzl", _dx_julia_toolchains = "dx_julia_toolchains")

julia = _pkg

julia_toolchain = _julia_toolchain

julia_runtime = _dx_julia_toolchains
