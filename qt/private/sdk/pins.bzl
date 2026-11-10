"""Pinned Qt SDK, runtime closure and extractor identities; versions, mirrors and license texts are in pins.json."""

load("@dx_pins_data//:pins.bzl", "DATA")
load("//core:seven_zip.bzl", "SEVEN_ZIP_EXTRACTORS")
load(":layout_linux.bzl", LINUX_LAYOUT = "LAYOUT")
load(":layout_macos.bzl", MACOS_LAYOUT = "LAYOUT")
load(":layout_windows.bzl", WINDOWS_LAYOUT = "LAYOUT")
load(":linux_arm64.bzl", LINUX_ARM64 = "CELL", LINUX_ARM64_IDENTITY = "IDENTITY")
load(":linux_x86_64.bzl", LINUX_X86_64 = "CELL", LINUX_X86_64_IDENTITY = "IDENTITY")
load(":macos_arm64.bzl", MACOS_ARM64 = "CELL", MACOS_ARM64_IDENTITY = "IDENTITY")
load(":windows_arm64.bzl", WINDOWS_ARM64 = "CELL", WINDOWS_ARM64_IDENTITY = "IDENTITY")
load(":windows_x86_64.bzl", WINDOWS_X86_64 = "CELL", WINDOWS_X86_64_IDENTITY = "IDENTITY")

_PINS = DATA["dx_qt_sdk"]

QT_VERSION = _PINS["version"]

QT_MIRROR = _PINS["qt_mirror"]

RUNTIME_MIRROR = _PINS["runtime_mirror"]

RUNTIME_REPOSITORIES = ["AppStream", "BaseOS"]

CLOSURES = ["core", "display", "gui"]

QT_LIBRARY_LICENSE = "LicenseRef-Qt-Commercial OR LGPL-3.0-only OR GPL-2.0-only OR GPL-3.0-only"

QT_TOOL_LICENSE = "LicenseRef-Qt-Commercial OR GPL-3.0-only WITH Qt-GPL-exception-1.0"

MODULES = {
    "qtbase": struct(
        files = [],
        requires = [],
        targets = ["core", "display_cocoa", "display_wayland", "display_xcb", "fonts", "gui", "metatypes", "moc", "network", "platforms", "plugins_imageformats", "plugins_platformthemes", "plugins_tls", "rcc", "test", "uic", "widgets"],
    ),
    "qtdeclarative": struct(
        files = [],
        requires = ["qtbase"],
        targets = ["qml", "qml_imports", "qmlformat", "qmllint", "qmltyperegistrar", "qtqml_qmldir", "quick", "quick_test"],
    ),
    "qttools": struct(
        files = ["bin/lrelease", "modules/Linguist.json", "sbom/qttools-" + QT_VERSION + ".spdx.json"],
        requires = ["qtdeclarative"],
        targets = ["lrelease"],
    ),
}

DEFAULT_MODULES = ["qtbase"]

RUNTIME_PLUGINS = LINUX_LAYOUT.runtime_plugins

LAYOUTS = {"linux": LINUX_LAYOUT, "macos": MACOS_LAYOUT, "windows": WINDOWS_LAYOUT}

CELLS = {
    "linux_arm64": LINUX_ARM64,
    "linux_x86_64": LINUX_X86_64,
    "macos_arm64": MACOS_ARM64,
    "windows_arm64": WINDOWS_ARM64,
    "windows_x86_64": WINDOWS_X86_64,
}

IDENTITIES = {
    "linux_arm64": LINUX_ARM64_IDENTITY,
    "linux_x86_64": LINUX_X86_64_IDENTITY,
    "macos_arm64": MACOS_ARM64_IDENTITY,
    "windows_arm64": WINDOWS_ARM64_IDENTITY,
    "windows_x86_64": WINDOWS_X86_64_IDENTITY,
}

EXTRACTORS = SEVEN_ZIP_EXTRACTORS

LICENSE_TEXTS = {
    key: struct(sha256 = entry["sha256"], url = entry["urls"][0])
    for key, entry in _PINS["archives"].items()
}
