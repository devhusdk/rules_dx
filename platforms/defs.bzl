"""Platform names, platform_select, and the providers and transition that language rules use to build for a platform."""

load("//platforms/private:names.bzl", _OS_NAMES = "OS_NAMES", _PLATFORMS = "PLATFORMS", _platforms_error = "platforms_error")
load("//platforms/private:select.bzl", _platform_select = "platform_select")
load(
    "//platforms/private:target.bzl",
    _DxPlatformInfo = "DxPlatformInfo",
    _DxTargetInfo = "DxTargetInfo",
    _platform_transition = "platform_transition",
)

DxPlatformInfo = _DxPlatformInfo
DxTargetInfo = _DxTargetInfo
PLATFORM_NAMES = sorted(_PLATFORMS)
OS_NAMES = _OS_NAMES
platform_select = _platform_select
platform_transition = _platform_transition
platforms_error = _platforms_error
