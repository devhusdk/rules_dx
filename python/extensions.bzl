"""Public Python module extensions."""

load("@rules_py//uv:extensions.bzl", _uv = "uv")
load("//python/private:sdist_cargo_vendor.bzl", _sdist_cargo_vendor = "sdist_cargo_vendor")

sdist_cargo_vendor = _sdist_cargo_vendor
uv = _uv
