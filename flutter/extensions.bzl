"""Public Flutter module extensions."""

load("@rules_flutter//flutter:extensions.bzl", _flutter = "flutter")
load("@rules_flutter//flutter/pub:extensions.bzl", _pub = "pub")

flutter = _flutter
pub = _pub
