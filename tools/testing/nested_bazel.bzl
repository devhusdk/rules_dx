"""Tags and platform constraints for tests that run a nested Bazel server."""

NESTED_BAZEL_TAGS = [
    "local",
    "no-sandbox",
    "resources:nested_bazel:1",
]

NESTED_BAZEL_COMPATIBLE_WITH = select({
    "@platforms//os:windows": ["@platforms//:incompatible"],
    "//conditions:default": [],
})
