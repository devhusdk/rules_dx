"""Public Julia module extensions."""

load("@rules_julia//julia/pkg:extensions.bzl", _pkg = "pkg")

julia = _pkg
