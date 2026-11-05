"""Starter test macros."""

load("//rust/rules:defs.bzl", "rust_test")
load("//tools/testing:nested_bazel.bzl", "STARTER_CONSUMER_TAGS")

def starter_consumer_test(name, fixture = None, target_compatible_with = []):
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
            "DX_SOURCE_ROOT": ".",
        },
        size = "small",
        tags = STARTER_CONSUMER_TAGS,
        target_compatible_with = target_compatible_with,
        deps = [
            "//tools/testing/starters:starter_scaffold",
            "//tools/testing:dx_testing",
        ],
    )

def starter_goldens_test(name):
    """Checks that every starter_consumer_test declared in this package has a golden listing, and the reverse."""
    declared = [
        test
        for test, attrs in native.existing_rules().items()
        if "starter-consumer" in attrs.get("tags", []) and not test.endswith("_upstream")
    ]
    goldens = [g[len("golden/"):-len(".txt")] for g in native.glob(["golden/*.txt"])]
    rust_test(
        name = name,
        srcs = ["//tools/testing/starters:starter_goldens_test.rs"],
        env = {
            "DX_DECLARED": " ".join(sorted(declared)),
            "DX_GOLDENS": " ".join(sorted(goldens)),
        },
    )
