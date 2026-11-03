"""The starter consumer test macro."""

load("//tools/testing:nested_bazel.bzl", "NESTED_BAZEL_ENV_INHERIT", "STARTER_CONSUMER_TAGS")
load("//tools/testing/hermetic:defs.bzl", "nested_bazel_test")

def starter_consumer_test(name, fixture = None, env_inherit = None, **kwargs):
    """Declares a manual consumer test that runs dx init plus one hello-world fixture."""
    fixture = fixture or name
    files = "//tools/testing/starters/fixtures:" + fixture
    nested_bazel_test(
        name = name,
        size = "enormous",
        srcs = ["//tools/testing/starters:starter_consumer_test.rs"],
        crate_root = "//tools/testing/starters:starter_consumer_test.rs",
        data = [
            "//:module_srcs",
            "//cli/cli:dx",
            files,
        ],
        env = {
            "DX_BIN": "$(rootpath //cli/cli:dx)",
            "DX_FIXTURE": fixture,
            "DX_FIXTURE_FILES": "$(rootpaths %s)" % files,
        },
        env_inherit = NESTED_BAZEL_ENV_INHERIT + (env_inherit or []),
        network = True,
        tags = STARTER_CONSUMER_TAGS,
        deps = [
            "//tools/testing:dx_nested",
            "//tools/testing:dx_testing",
        ],
        **kwargs
    )
