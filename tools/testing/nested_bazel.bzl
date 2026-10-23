"""Tags for tests that run a nested Bazel server."""

NESTED_BAZEL_TAGS = [
    "local",
    "no-sandbox",
    "resources:nested_bazel:1",
]
