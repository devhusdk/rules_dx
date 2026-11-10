"""Pinned cosign release and its per-platform checksums, read from pins.json."""

load("@dx_pins_data//:pins.bzl", "DATA")

COSIGN_VERSION = DATA["dx_cosign"]["version"]

COSIGN_PLATFORMS = DATA["dx_cosign"]["cells"]
