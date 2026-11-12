"""Default Xcode version and deployment targets of the Apple app rules; the data is in apple/pins.json."""

load("@dx_pins_data//:pins.bzl", "DATA")

_PINS = DATA["dx_apple"]

XCODE_VERSION = _PINS["xcode_version"]

IOS_DEPLOYMENT = _PINS["deployment"]["ios"]

MACOS_DEPLOYMENT = _PINS["deployment"]["macos"]
