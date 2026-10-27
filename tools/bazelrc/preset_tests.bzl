"""Preset freshness tests."""

load("//libs/starlark:defs.bzl", "starlark_test")

def preset_update_tests(name):
    """Declare the vendored-preset freshness test."""
    starlark_test(
        name = name,
        mode = "execution",
        file_checks = {
            "//:.bazelversion": "9.2.0",
            "//:.bazelrc": "import %workspace%/tools/bazelrc/correctness.bazelrc\nimport %workspace%/tools/bazelrc/performance.bazelrc\nimport %workspace%/tools/bazelrc/resources.bazelrc\ntry-import %workspace%/user.bazelrc",
            ":preset": "GENERATED, do not edit.\n# Regenerate: `tools/dx init --apply`, or `bazel run //tools/bazelrc:preset_update` in rules_dx.",
            ":src/lib.rs": "PRESET_BAZEL_VERSION",
        },
    )
