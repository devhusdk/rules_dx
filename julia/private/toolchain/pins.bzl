"""Pinned rules_julia identity; the archive data is in pins.json."""

load("@dx_pins_data//:pins.bzl", "DATA")

_PINS = DATA["dx_julia"]

RULES_JULIA = struct(
    sha256 = _PINS["archives"]["rules_julia"]["sha256"],
    urls = _PINS["archives"]["rules_julia"]["urls"],
)
