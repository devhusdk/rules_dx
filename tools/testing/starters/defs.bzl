"""Starter test macros."""

load("//rust:defs.bzl", "rust_test")
load("//tools/testing:nested_bazel.bzl", "STARTER_CONSUMER_TAGS")

def starter_consumer_test(name, *, fixture = None, target_compatible_with = None, bazel_deps = ["rules_dx"]):
    """Declares a test that checks one starter fixture's generated files and its MODULE.bazel bazel_dep names."""
    target_compatible_with = target_compatible_with or []
    fixture = fixture or name
    files = "//tools/testing/starters/fixtures:" + fixture
    rust_test(
        name = name,
        srcs = ["//tools/testing/starters:starter_template_test.rs"],
        crate_root = "//tools/testing/starters:starter_template_test.rs",
        data = [
            "//cli/cli:dx",
            files,
        ],
        env = {
            "DX_BIN": "$(rootpath //cli/cli:dx)",
            "DX_FIXTURE": fixture,
            "DX_FIXTURE_FILES": "$(rootpaths %s)" % files,
            "DX_BAZEL_DEPS": " ".join(bazel_deps),
            "DX_SOURCE_ROOT": ".",
        },
        size = "small",
        tags = STARTER_CONSUMER_TAGS,
        target_compatible_with = target_compatible_with,
        deps = [
            "//tools/testing/starters:starter_scaffold",
        ],
    )
