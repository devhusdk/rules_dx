"""Pinned Windows SDK Build Tools identity; the archive data is in pins.json."""

load("@dx_pins_data//:pins.bzl", "DATA")

_BUILDTOOLS = DATA["dx_msix"]["archives"]["buildtools"]

BUILDTOOLS_VERSION = _BUILDTOOLS["version"]

BUILDTOOLS_BIN = ".".join(BUILDTOOLS_VERSION.split(".")[:3]) + ".0"

BUILDTOOLS_URL = _BUILDTOOLS["urls"][0]

BUILDTOOLS_SHA256 = _BUILDTOOLS["sha256"]

LICENSE = "windows-sdk-buildtools"

LICENSE_URL = "https://aka.ms/WinSDKLicenseURL"

TOOL_ARCHES = ["arm64", "x64"]

SUPPORTED_CELLS = ["windows_arm64", "windows_x86_64"]

MIN_VERSION = "10.0.17763.0"

MAX_VERSION_TESTED = "10.0.26100.0"
