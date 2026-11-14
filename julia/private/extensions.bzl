"""The rules_julia toolchain and package extensions, loaded from the patched rules_julia."""

load("@rules_julia//julia:extensions.bzl", _julia = "julia")
load("@rules_julia//julia/pkg:extensions.bzl", _pkg = "pkg")

julia = _julia

pkg = _pkg
