"""Xcode version and deployment floors shared by the Apple app and library rules."""

load(":validate.bzl", "SDK_FLOORS", _XCODE_LINEAGE = "XCODE_LINEAGE")

XCODE_LINEAGE = _XCODE_LINEAGE

_SDK_CEILINGS = {
    "iphoneos": "18.0",
    "iphonesimulator": "18.0",
    "macosx": "15.0",
}

def deployment_floor(sdk):
    """Returns the minimum deployment target for one SDK, or empty."""
    return SDK_FLOORS[sdk] if sdk in SDK_FLOORS else ""

def ceiling_for(sdk):
    """Returns the Xcode 16 deployment ceiling for one SDK, or empty."""
    return _SDK_CEILINGS[sdk] if sdk in _SDK_CEILINGS else ""

def version_matches(declared, resolved):
    """Returns whether a resolved version carries every declared component."""
    wanted = declared.split(".")
    actual = resolved.split(".")
    for index in range(len(wanted)):
        have = actual[index] if index < len(actual) else "0"
        if have.isdigit() and wanted[index].isdigit():
            if int(have) != int(wanted[index]):
                return False
        elif have != wanted[index]:
            return False
    return True
