"""Tags and platform constraints for tests that run a nested Bazel server."""

NESTED_BAZEL_TAGS = [
    "local",
    "no-sandbox",
    "resources:cpu:2",
    "resources:memory:2560",
    "resources:nested_bazel:1",
]

NESTED_BAZEL_NETWORK_TAGS = NESTED_BAZEL_TAGS + ["requires-network"]

# Opts a test into the CI `network` job, which selects the `network-consumer` tag.
NESTED_BAZEL_CI_NETWORK_TAGS = NESTED_BAZEL_NETWORK_TAGS + ["network-consumer"]

_NESTED_BAZEL_BIN = str(Label("//tools/testing:nested_bazel_bin"))

_NESTED_BAZEL_SHIM = str(Label("//tools/testing:nested_bazel_shim"))

NESTED_BAZEL_DATA = [_NESTED_BAZEL_BIN, _NESTED_BAZEL_SHIM]

NESTED_BAZEL_ENV = {
    "DX_NESTED_BAZEL_PINNED": "$(rlocationpath " + _NESTED_BAZEL_BIN + ")",
    "DX_NESTED_BAZEL_SHIM": "$(rlocationpath " + _NESTED_BAZEL_SHIM + ")",
}

_CONSUMER_MAVEN_LOCK = str(Label("//tools/testing/fixtures/consumer_maven:maven_install.json"))

# The pinned Maven lock that nested consumers copy into their workspace.
CONSUMER_MAVEN_DATA = [_CONSUMER_MAVEN_LOCK]

CONSUMER_MAVEN_ENV = {
    "DX_CONSUMER_MAVEN_LOCK": "$(rlocationpath " + _CONSUMER_MAVEN_LOCK + ")",
}

# The host variables a nested Bazel client needs on Linux, macOS and Windows.
NESTED_BAZEL_ENV_INHERIT = [
    "APPDATA",
    "BAZELISK_BASE_URL",
    "BAZELISK_HOME",
    "HOME",
    "LOCALAPPDATA",
    "PATH",
    "ProgramData",
    "SystemRoot",
    "TEMP",
    "TMP",
    "USERPROFILE",
    "XDG_CACHE_HOME",
]

NESTED_BAZEL_NETWORK_ENV = NESTED_BAZEL_ENV | {"DX_NESTED_BAZEL_NETWORK": "1"}

# Nested Bazel runs everywhere a pinned binary exists; kept as a symbol so callers need not change.
NESTED_BAZEL_COMPATIBLE_WITH = []
