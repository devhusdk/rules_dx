"""Pinned AppImage tool and runtime identities; the archive data is in pins.json."""

load("@dx_pins_data//:pins.bzl", "DATA")

_ARCHIVES = DATA["dx_appimage"]["archives"]

TOOL_VERSION = _ARCHIVES["appimagetool_x86_64"]["version"]

TOOLS = {
    arch: {"sha256": _ARCHIVES["appimagetool_" + arch]["sha256"], "url": _ARCHIVES["appimagetool_" + arch]["urls"][0]}
    for arch in ["aarch64", "x86_64"]
}

TOOL_LICENSE = "MIT"

RUNTIME_VERSION = _ARCHIVES["runtime_x86_64"]["version"]

RUNTIMES = {
    arch: {"sha256": _ARCHIVES["runtime_" + arch]["sha256"], "url": _ARCHIVES["runtime_" + arch]["urls"][0]}
    for arch in ["aarch64", "x86_64"]
}

RUNTIME_LICENSE = "MIT"

MKSQUASHFS_VERSION = "4.6.1"

MKSQUASHFS_LICENSE = "GPL-2.0-or-later"

TOOL_BINARIES = ["appimagetool", "desktop-file-validate", "mksquashfs"]

ARCHES = ["aarch64", "x86_64"]

SUPPORTED_CELLS = ["linux_arm64", "linux_x86_64"]
