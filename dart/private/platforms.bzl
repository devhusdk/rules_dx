"""The platforms rules_dart ships a native SDK for."""

load("//core:platform_support.bzl", "lang_platforms", "os_cpu_support")

DART_PLATFORMS = lang_platforms("dart")

PLATFORM_SUPPORT = os_cpu_support("//dart/private:defs.bzl", "DART_COMPATIBLE_WITH", DART_PLATFORMS, [Label("//dart/private:rules_dart_platform")], "//dart/private")
