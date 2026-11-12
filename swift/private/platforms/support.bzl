"""The platforms the Swift wrappers build for."""

load("//core:platform_support.bzl", "compatible_with", "platform_support")

PLATFORM_SUPPORT = platform_support(
    "//swift/private:defs.bzl",
    "SWIFT_COMPATIBLE_WITH",
    [
        ["@platforms//os:ios"],
        ["@platforms//os:linux", "@platforms//cpu:aarch64"],
        ["@platforms//os:linux", "@platforms//cpu:x86_64"],
        ["@platforms//os:macos"],
        ["@platforms//os:windows", "@platforms//cpu:aarch64"],
        ["@platforms//os:windows", "@platforms//cpu:x86_64"],
    ],
    [
        Label("@platforms//os:ios"),
        Label("//swift/private/platforms:is_linux_aarch64"),
        Label("//swift/private/platforms:is_linux_x86_64"),
        Label("@platforms//os:macos"),
        Label("//swift/private/platforms:is_windows_aarch64"),
        Label("//swift/private/platforms:is_windows_x86_64"),
    ],
)

SWIFT_COMPATIBLE_WITH = compatible_with(PLATFORM_SUPPORT)

SWIFT_FUZZ_COMPATIBLE_WITH = ["@platforms//os:linux"]
