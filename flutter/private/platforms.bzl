"""The host platforms rules_flutter ships a Flutter SDK for and rules_dx advertises."""

load("//core:platform_support.bzl", "os_cpu_support")

FLUTTER_PLATFORMS = {
    "linux_x86_64": ("linux", "x86_64"),
    "linux_arm64": ("linux", "arm64"),
    "macos_arm64": ("macos", "arm64"),
    "windows_x86_64": ("windows", "x86_64"),
}

PLATFORM_SUPPORT = os_cpu_support("//flutter:defs.bzl", "FLUTTER_COMPATIBLE_WITH", FLUTTER_PLATFORMS, [Label("//flutter/private:rules_flutter_platform"), Label("@platforms//os:android"), Label("@platforms//os:ios")], "//flutter/private")
