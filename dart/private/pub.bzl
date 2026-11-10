"""The rules_dart pub extension, patched to extract archives with a pinned bsdtar."""

load("@dx_rules_dart_pub//dart/pub:extensions.bzl", _pub = "pub")

pub = _pub
