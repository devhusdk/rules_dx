"""Public Flutter module extensions."""

load("@rules_flutter//flutter:extensions.bzl", _flutter = "flutter")
load("//flutter/private:pub.bzl", _pub = "pub")

flutter = _flutter
pub = _pub
