"""Public Android module extensions and repository rules."""

load("@rules_android//rules/android_sdk_repository:rule.bzl", _android_sdk_repository = "android_sdk_repository_extension")
load("//android/private/sdk:extensions.bzl", _android = "android")
load("//android/private/toolchain:repos.bzl", _android_ndk_toolchain_repo = "android_ndk_toolchain_repo")

android_ndk_toolchain_repo = _android_ndk_toolchain_repo
android = _android
android_sdk_repository = _android_sdk_repository
