"""The platforms rules_dart ships a native SDK for."""

load("//core:platform_support.bzl", "os_cpu_support")

DART_PLATFORMS = {
    "linux_x86_64": ("linux", "x86_64"),
    "linux_arm64": ("linux", "arm64"),
    "macos_arm64": ("macos", "arm64"),
    "windows_x86_64": ("windows", "x86_64"),
}

PLATFORM_SUPPORT = os_cpu_support("//dart:defs.bzl", "DART_COMPATIBLE_WITH", DART_PLATFORMS, [Label("//dart/private:rules_dart_platform")], "//dart/private")
