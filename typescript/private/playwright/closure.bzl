"""Ubuntu 22.04 library closure the Chromium headless shell loads on Linux; the data is in typescript/private/playwright/pins.json."""

load("//core:jammy_runtime.bzl", "JAMMY_RUNTIME")

visibility(["//..."])

CLOSURE_DIR = "closure"

_SHARED = ["gcc-12-base", "libgcc-s1", "zlib1g"]

def chromium_closure(pins, platform):
    """Returns the .deb packages of one platform's Chromium closure in `pins`, the decoded pins.json."""
    if platform not in pins["closure"]:
        return []
    shared = [package for package in JAMMY_RUNTIME[platform] if package.package in _SHARED]
    packages = pins["closure"][platform]
    return shared + [
        struct(
            copyright = packages[name]["copyright"],
            deb = packages[name]["deb"],
            libraries = packages[name]["libraries"],
            package = name,
            pool = packages[name]["pool"],
            sha256 = packages[name]["sha256"],
        )
        for name in sorted(packages)
    ]
