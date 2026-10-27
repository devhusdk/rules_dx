"""Pinned Qt archives of the macos_arm64 cell."""

load(":layout_macos.bzl", "LAYOUT")

_RELEASE = "6.11.3-0-202610220312"

_SUFFIX = "-MacOS-MacOS_15-Clang-MacOS-MacOS_15-X86_64-ARM64.7z"

CELL = struct(
    archives = {
        "qtbase": struct(
            archive = _RELEASE + "qtbase" + _SUFFIX,
            sha256 = "fb9e4a68f2d217dcea330208a79b087b3f729cc7118ff2ca40be193cf80c7606",
        ),
        "qtdeclarative": struct(
            archive = _RELEASE + "qtdeclarative" + _SUFFIX,
            sha256 = "fc5b473993cc148e05209de888df35b217175a3d08e2dfac856d0bb70394bb2e",
        ),
        "qttools": struct(
            archive = _RELEASE + "qttools" + _SUFFIX,
            sha256 = "95a53ddafffcaea6f01f2ea1e4b1aed62b6c868188ce06d4edf15c5233dfb2a4",
        ),
    },
    blocker = "",
    constraints = ["@platforms//os:macos", "@platforms//cpu:arm64"],
    directory = "mac_x64/desktop/qt6_6113/qt6_6113/qt.qt6.6113.clang_64",
    layout = LAYOUT,
    needs = None,
    release = _RELEASE,
    runtime = None,
)
