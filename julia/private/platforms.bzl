"""The platforms Julia ships a runtime for."""

load("//core:platform_support.bzl", "lang_platforms", "os_cpu_support")

JULIA_PLATFORMS = lang_platforms("julia")

PLATFORM_SUPPORT = os_cpu_support("//julia/private:defs.bzl", "JULIA_COMPATIBLE_WITH", JULIA_PLATFORMS, [Label("//julia/private:rules_julia_platform")], "//julia/private")
