"""The platforms rules_perl ships a toolchain for."""

load("//core:platform_support.bzl", "os_cpu_support")

PERL_PLATFORMS = {
    "linux_x86_64": ("linux", "x86_64"),
    "linux_arm64": ("linux", "arm64"),
    "macos_x86_64": ("macos", "x86_64"),
    "macos_arm64": ("macos", "arm64"),
    "windows_x86_64": ("windows", "x86_64"),
}

PLATFORM_SUPPORT = os_cpu_support("//perl:defs.bzl", "PERL_COMPATIBLE_WITH", PERL_PLATFORMS, [Label("//perl/private:rules_perl_platform")], "//perl/private")
