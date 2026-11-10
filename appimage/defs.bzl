"""Public AppImage API."""

load(
    "//appimage/private:defs.bzl",
    _AppImageInfo = "AppImageInfo",
    _appimage_package = "appimage_package",
)

AppImageInfo = _AppImageInfo
appimage_package = _appimage_package
