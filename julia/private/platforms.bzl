"""The platforms Julia ships a runtime for."""

load("//core:platform_support.bzl", "os_cpu_support")

JULIA_PLATFORMS = {
    "linux_x86_64": ("linux", "x86_64"),
    "linux_arm64": ("linux", "arm64"),
    "macos_arm64": ("macos", "arm64"),
    "windows_x86_64": ("windows", "x86_64"),
}

PLATFORM_SUPPORT = os_cpu_support("//julia:defs.bzl", "JULIA_COMPATIBLE_WITH", JULIA_PLATFORMS, [Label("//julia/private:rules_julia_platform")], "//julia/private")
