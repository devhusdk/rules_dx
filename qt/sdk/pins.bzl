"""Pinned Qt SDK, runtime closure and extractor identities."""

load("//libs/starlark:seven_zip.bzl", "SEVEN_ZIP_EXTRACTORS")
load(":layout_linux.bzl", LINUX_LAYOUT = "LAYOUT")
load(":layout_macos.bzl", MACOS_LAYOUT = "LAYOUT")
load(":layout_windows.bzl", WINDOWS_LAYOUT = "LAYOUT")
load(":linux_arm64.bzl", LINUX_ARM64 = "CELL", LINUX_ARM64_IDENTITY = "IDENTITY")
load(":linux_x86_64.bzl", LINUX_X86_64 = "CELL", LINUX_X86_64_IDENTITY = "IDENTITY")
load(":macos_arm64.bzl", MACOS_ARM64 = "CELL", MACOS_ARM64_IDENTITY = "IDENTITY")
load(":windows_arm64.bzl", WINDOWS_ARM64 = "CELL", WINDOWS_ARM64_IDENTITY = "IDENTITY")
load(":windows_x86_64.bzl", WINDOWS_X86_64 = "CELL", WINDOWS_X86_64_IDENTITY = "IDENTITY")

QT_VERSION = "6.11.3"

QT_MIRROR = "https://download.qt.io/online/qtsdkrepository"

ICU_VERSION = "73.2"

RUNTIME_MIRROR = "https://vault.almalinux.org"

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
    "icu/LICENSE": struct(
        sha256 = "f3005e195ff74d8812cc1f182a1c446fab678d70a10e3dada497585befee5416",
        url = "https://raw.githubusercontent.com/unicode-org/icu/release-73-2/icu4c/LICENSE",
    ),
    "qt/GPL-2.0-only.txt": struct(
        sha256 = "8177f97513213526df2cf6184d8ff986c675afb514d4e68a404010521b880643",
        url = "https://raw.githubusercontent.com/qt/qtbase/5a1194b2d368e2d72c230205c464b04a2f23eccd/LICENSES/GPL-2.0-only.txt",
    ),
    "qt/GPL-3.0-only.txt": struct(
        sha256 = "8ceb4b9ee5adedde47b31e975c1d90c73ad27b6b165a1dcd80c7c545eb65b903",
        url = "https://raw.githubusercontent.com/qt/qtbase/5a1194b2d368e2d72c230205c464b04a2f23eccd/LICENSES/GPL-3.0-only.txt",
    ),
    "qt/LGPL-3.0-only.txt": struct(
        sha256 = "da7eabb7bafdf7d3ae5e9f223aa5bdc1eece45ac569dc21b3b037520b4464768",
        url = "https://raw.githubusercontent.com/qt/qtbase/5a1194b2d368e2d72c230205c464b04a2f23eccd/LICENSES/LGPL-3.0-only.txt",
    ),
    "qt/LicenseRef-Qt-Commercial.txt": struct(
        sha256 = "9b1f50aae6267f9d5e0ceb6775ee86450262c25ec7c0573e151fe5d3f18a4700",
        url = "https://raw.githubusercontent.com/qt/qtbase/5a1194b2d368e2d72c230205c464b04a2f23eccd/LICENSES/LicenseRef-Qt-Commercial.txt",
    ),
    "qt/Qt-GPL-exception-1.0.txt": struct(
        sha256 = "40678d338ce53cd93f8b22b281a2ecbcaa3ee65ce60b25ffb0c462b0530846b2",
        url = "https://raw.githubusercontent.com/qt/qtbase/5a1194b2d368e2d72c230205c464b04a2f23eccd/LICENSES/Qt-GPL-exception-1.0.txt",
    ),
}
