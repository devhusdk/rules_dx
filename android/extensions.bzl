"""Public Android module extensions."""

load("@rules_android//rules/android_sdk_repository:rule.bzl", _android_sdk_repository = "android_sdk_repository_extension")
load("//android/private/sdk:extensions.bzl", _android = "android")

android = _android
android_sdk_repository = _android_sdk_repository
