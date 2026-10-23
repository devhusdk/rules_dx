"""Pinned Dart foundation."""

load(":versions.bzl", _DART_SDK_VERSION = "DART_SDK_VERSION", _RULES_DART_VERSION = "RULES_DART_VERSION")

RULES_DART_VERSION = _RULES_DART_VERSION
DART_SDK_VERSION = _DART_SDK_VERSION

PUBSPEC = "//third_party/dart:pubspec.yaml"
PUBSPEC_LOCK = "//third_party/dart:pubspec.lock"
PUB_HUB = "@dx_dart_deps"
