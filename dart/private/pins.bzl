"""Pinned rules_dart archive for the pub extension."""

load("//modules:versions.bzl", "RULES_DART_VERSION")

RULES_DART_SHA256 = "e7d9271bb10e5ed0a20415a70a120b367a22d8dd7afe7b0368810f1a08f43170"

RULES_DART_STRIP_PREFIX = "rules_dart-" + RULES_DART_VERSION

RULES_DART_URL = "https://github.com/aran/rules_dart/releases/download/v" + RULES_DART_VERSION + "/rules_dart-v" + RULES_DART_VERSION + ".tar.gz"
