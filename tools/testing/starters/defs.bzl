"""Starter test macros."""

load("//rust:defs.bzl", "rust_test")

def starter_consumer_test(name, *, bazel_deps):
    """Declares one test that checks every starter fixture's generated files and its MODULE.bazel bazel_dep names."""
    fixtures = sorted(bazel_deps)
    env = {
        "DX_BIN": "$(rootpath //cli/cli:dx)",
        "DX_STARTERS": ";".join([fixture + ":" + " ".join(bazel_deps[fixture]) for fixture in fixtures]),
    }
    for fixture in fixtures:
        env["DX_FIXTURE_FILES_" + fixture.replace("-", "_")] = "$(rootpaths //tools/testing/starters/fixtures:%s)" % fixture
    rust_test(
        name = name,
        srcs = ["//tools/testing/starters:starter_template_test.rs"],
        crate_root = "//tools/testing/starters:starter_template_test.rs",
        data = ["//cli/cli:dx"] + ["//tools/testing/starters/fixtures:" + fixture for fixture in fixtures],
        env = env,
        size = "small",
        tags = ["starter-consumer"],
        deps = [
            "//cli/test_scratch:dx_test_scratch",
            "//tools/testing/starters:starter_scaffold",
        ],
    )
