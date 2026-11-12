"""The host platforms rules_flutter ships a Flutter SDK for and rules_dx advertises."""

load("//core:platform_support.bzl", "lang_platforms", "os_cpu_support")

FLUTTER_PLATFORMS = lang_platforms("flutter")

PLATFORM_SUPPORT = os_cpu_support("//flutter/private:defs.bzl", "FLUTTER_COMPATIBLE_WITH", FLUTTER_PLATFORMS, [Label("//flutter/private:rules_flutter_platform"), Label("@platforms//os:android"), Label("@platforms//os:ios")], "//flutter/private")
