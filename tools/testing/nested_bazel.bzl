"""Tags and platform constraints for tests that run a nested Bazel server."""

load("//core/modules:lock_registry.bzl", "LOCK_REGISTRY")

NESTED_BAZEL_TAGS = [
    "resources:cpu:2",
    "resources:memory:2560",
    "resources:nested_bazel:1",
]

NESTED_BAZEL_NETWORK_TAGS = NESTED_BAZEL_TAGS + ["requires-network", "network-consumer"]

_NESTED_BAZEL_BIN = str(Label("//tools/testing:nested_bazel_bin"))

_NESTED_BAZEL_SHIM = str(Label("//tools/testing:nested_bazel_shim"))

NESTED_BAZEL_DATA = [_NESTED_BAZEL_BIN, _NESTED_BAZEL_SHIM]

NESTED_BAZEL_ENV = {
    "DX_NESTED_BAZEL_PINNED": "$(rlocationpath " + _NESTED_BAZEL_BIN + ")",
    "DX_NESTED_BAZEL_SHIM": "$(rlocationpath " + _NESTED_BAZEL_SHIM + ")",
}

_CONSUMER_MAVEN_LOCK = str(Label("//tools/testing/fixtures/consumer_maven:" + LOCK_REGISTRY["maven"]["lock"]))

CONSUMER_MAVEN_DATA = [_CONSUMER_MAVEN_LOCK]

CONSUMER_MAVEN_ENV = {
    "DX_CONSUMER_MAVEN_LOCK": "$(rlocationpath " + _CONSUMER_MAVEN_LOCK + ")",
}

NESTED_BAZEL_NETWORK_ENV = NESTED_BAZEL_ENV | {"DX_NESTED_BAZEL_NETWORK": "1"}
