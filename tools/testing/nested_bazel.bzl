"""Tags and platform constraints for tests that run a nested Bazel server."""

NESTED_BAZEL_TAGS = [
    "local",
    "no-sandbox",
    "resources:cpu:2",
    "resources:memory:2560",
    "resources:nested_bazel:1",
]

NESTED_BAZEL_NETWORK_TAGS = NESTED_BAZEL_TAGS + ["requires-network"]

_NESTED_BAZEL_BIN = str(Label("//tools/testing:nested_bazel_bin"))

NESTED_BAZEL_DATA = [_NESTED_BAZEL_BIN]

NESTED_BAZEL_ENV = {"DX_NESTED_BAZEL_PINNED": "$(rlocationpath " + _NESTED_BAZEL_BIN + ")"}

NESTED_BAZEL_NETWORK_ENV = NESTED_BAZEL_ENV | {"DX_NESTED_BAZEL_NETWORK": "1"}

NESTED_BAZEL_COMPATIBLE_WITH = select({
    "@platforms//os:windows": ["@platforms//:incompatible"],
    "//conditions:default": [],
})
