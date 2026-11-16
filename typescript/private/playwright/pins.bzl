"""Playwright and Chromium pins of typescript_playwright_test; the data is in typescript/private/playwright/pins.json."""

load("@dx_pins_data//:pins.bzl", "DATA")

visibility(["//..."])

_PINS = DATA["dx_playwright"]

PLAYWRIGHT_VERSION = _PINS["playwright"]

CHROMIUM_REVISION = _PINS["revision"]

CHROMIUM_VERSION = _PINS["version"]

def _constraints(platform):
    os, cpu = platform.split("_", 1)
    return ["@platforms//os:" + os, "@platforms//cpu:" + cpu]

ARTIFACTS = {platform: struct(constraints = _constraints(platform)) for platform in _PINS["cells"]}

def playwright_compatible_with():
    """Returns the target_compatible_with select that accepts only platforms with a pinned Chromium."""
    cases = {Label("@dx_playwright_chromium//:" + platform): [] for platform in sorted(ARTIFACTS.keys())}
    return select(cases | {"//conditions:default": [Label("@platforms//:incompatible")]})
