"""The platforms the Swift wrappers build for, from swift/lang.json plus iOS."""

load("//core:platform_support.bzl", "compatible_with", "lang_platforms", "platform_support")

_CPUS = {"arm64": "aarch64", "x86_64": "x86_64"}

SWIFT_PLATFORMS = {id: (os, _CPUS[cpu]) for id, (os, cpu) in lang_platforms("swift").items()}

PLATFORM_SUPPORT = platform_support(
    "//swift/private:defs.bzl",
    "SWIFT_COMPATIBLE_WITH",
    [["@platforms//os:ios"]] + [
        ["@platforms//os:" + os, "@platforms//cpu:" + cpu]
        for os, cpu in sorted(SWIFT_PLATFORMS.values())
    ],
    [Label("@platforms//os:ios")] + [
        Label("//swift/private/platforms:is_{}_{}".format(os, cpu))
        for os, cpu in sorted(SWIFT_PLATFORMS.values())
    ],
)

SWIFT_COMPATIBLE_WITH = compatible_with(PLATFORM_SUPPORT)

SWIFT_FUZZ_COMPATIBLE_WITH = ["@platforms//os:linux"]
