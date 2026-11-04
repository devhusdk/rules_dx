"""Pinned Qt archives of the windows_x86_64 cell."""

load(":layout_windows.bzl", "LAYOUT")

_RELEASE = "6.11.3-0-202610220312"

_SUFFIX = "-Windows-Windows_11_24H2-MSVC2022-Windows-Windows_11_24H2-X86_64.7z"

CELL = struct(
    archives = {
        "qtbase": struct(
            archive = _RELEASE + "qtbase" + _SUFFIX,
            sha256 = "68be61b613b963daf206834fdcc57dd3b84baff3082e50679e5846ca4a7a8b86",
        ),
        "qtdeclarative": struct(
            archive = _RELEASE + "qtdeclarative" + _SUFFIX,
            sha256 = "7c1bb872495144f154d24931b0b58f817defbfd4099b0ac99372062bdfd2ac81",
        ),
        "qttools": struct(
            archive = _RELEASE + "qttools" + _SUFFIX,
            sha256 = "b519f552424d8c748ed37c6b3a7615d29a1e3920383ee8ccf9b2066998341c2c",
        ),
    },
    blocker = "MSVC toolchain and Windows probe evidence pending (#4815, #5972)",
    constraints = ["@platforms//os:windows", "@platforms//cpu:x86_64"],
    directory = "windows_x86/desktop/qt6_6113/qt6_6113_msvc2022_64/qt.qt6.6113.win64_msvc2022_64",
    layout = LAYOUT,
    needs = None,
    release = _RELEASE,
    runtime = None,
)
