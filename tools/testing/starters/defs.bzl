"""Starter test macros."""

load("//rust/rules:defs.bzl", "rust_test")
load("//tools/testing:nested_bazel.bzl", "NESTED_BAZEL_COMPATIBLE_WITH", "STARTER_CONSUMER_TAGS")

def starter_consumer_test(name, fixture = None, target_compatible_with = None):
    """Declares a test that checks one starter fixture's generated files against a golden listing."""
    fixture = fixture or name
    files = "//tools/testing/starters/fixtures:" + fixture
    golden = "//tools/testing/starters:golden/%s.txt" % fixture
    rust_test(
        name = name,
        srcs = ["//tools/testing/starters:starter_template_test.rs"],
        crate_root = "//tools/testing/starters:starter_template_test.rs",
        data = [
            "//cli/cli:dx",
            files,
            golden,
        ],
        env = {
            "DX_BIN": "$(rootpath //cli/cli:dx)",
            "DX_FIXTURE": fixture,
            "DX_FIXTURE_FILES": "$(rootpaths %s)" % files,
            "DX_GOLDEN": "$(rootpath %s)" % golden,
        },
        size = "small",
        tags = STARTER_CONSUMER_TAGS,
        target_compatible_with = target_compatible_with or NESTED_BAZEL_COMPATIBLE_WITH,
        deps = [
            "//tools/testing:dx_nested",
            "//tools/testing:dx_testing",
        ],
    )
