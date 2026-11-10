"""Public D module extensions."""

load("//d/private/dub:extensions.bzl", _dub = "dub")
load("//d/private/toolchain:extensions.bzl", _d = "d")

d = _d
dub = _dub
