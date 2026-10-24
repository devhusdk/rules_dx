"""Fixture NDK repositories for the fetch test."""

load("//android/sdk:repos.bzl", "NDK_REPOSITORY_ATTRS", "acquire_ndk")

ndk_fixture_repo = repository_rule(
    implementation = acquire_ndk,
    attrs = NDK_REPOSITORY_ATTRS,
)
