"""Public Android repository rules."""

load("//android/private/toolchain:repos.bzl", _android_ndk_toolchain_repo = "android_ndk_toolchain_repo")

# Loaded from consumer MODULE.bazel files through use_repo_rule.
visibility("public")

android_ndk_toolchain_repo = _android_ndk_toolchain_repo
