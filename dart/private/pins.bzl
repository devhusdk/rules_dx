"""Pinned rules_dart archive for the pub extension; the archive data is in pins.json."""

load("@dx_pins_data//:pins.bzl", "DATA")

_RULES_DART = DATA["dx_dart"]["archives"]["rules_dart"]

RULES_DART_SHA256 = _RULES_DART["sha256"]

RULES_DART_STRIP_PREFIX = _RULES_DART["strip_prefix"]

RULES_DART_URL = _RULES_DART["urls"][0]
