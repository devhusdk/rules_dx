"""Tags and platform constraints for tests that run a nested Bazel server."""

NESTED_BAZEL_TAGS = [
    "local",
    "no-sandbox",
    "resources:cpu:2",
    "resources:memory:2048",
    "resources:nested_bazel:1",
]

NESTED_BAZEL_NETWORK_TAGS = NESTED_BAZEL_TAGS + ["requires-network"]

NESTED_BAZEL_NETWORK_ENV = {"DX_NESTED_BAZEL_NETWORK": "1"}

NESTED_BAZEL_COMPATIBLE_WITH = select({
    "@platforms//os:windows": ["@platforms//:incompatible"],
    "//conditions:default": [],
})
