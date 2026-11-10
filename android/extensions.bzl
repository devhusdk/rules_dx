"""Public Android module extensions."""

load("@rules_android//rules/android_sdk_repository:rule.bzl", _android_sdk_repository_extension = "android_sdk_repository_extension")
load("//android/private/sdk:extensions.bzl", _android_sdk = "android_sdk")

android_sdk = _android_sdk
android_sdk_repository_extension = _android_sdk_repository_extension
