"""Unit tests for the shell-harness helper."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load(":harness.bzl", "dx_harness_data", "dx_harness_entry", "dx_harness_env", "dx_harness_linux", "dx_harness_tags")

def harness_unit_tests(name):
    """Runs the unit tests for the shell-harness helper."""
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal(
                "bare data resolves to the shared bootstrap pair",
                dx_harness_data(),
                ["//tools/sh:bootstrap", "//tools/sh:lib"],
            ),
            expect_equal(
                "caller data keeps its entries ahead of the shared pair",
                dx_harness_data([":version"]),
                [":version", "//tools/sh:bootstrap", "//tools/sh:lib"],
            ),
            expect_equal(
                "bare tags resolve to the no-coverage marker",
                dx_harness_tags(),
                ["no-coverage"],
            ),
            expect_equal(
                "caller tags keep their entries ahead of the marker",
                dx_harness_tags(["requires-network"]),
                ["requires-network", "no-coverage"],
            ),
            expect_equal(
                "existing marker is not duplicated",
                dx_harness_tags(["no-coverage"]),
                ["no-coverage"],
            ),
            expect_equal(
                "bare constraint resolves to Linux-only",
                dx_harness_linux(),
                ["@platforms//os:linux"],
            ),
            expect_equal(
                "explicit constraint passes through untouched",
                dx_harness_linux(["@platforms//os:macos"]),
                ["@platforms//os:macos"],
            ),
            expect_equal(
                "bare env resolves to the bootstrap preload",
                dx_harness_env(),
                {"DX_BOOTSTRAP": "_main/tools/sh/bootstrap.sh"},
            ),
            expect_equal(
                "caller env keeps its entries plus the preload",
                dx_harness_env({"FOO": "bar"}),
                {"FOO": "bar", "DX_BOOTSTRAP": "_main/tools/sh/bootstrap.sh"},
            ),
            expect_equal(
                "explicit preload is not overwritten",
                dx_harness_env({"DX_BOOTSTRAP": "/custom/bootstrap.sh"}),
                {"DX_BOOTSTRAP": "/custom/bootstrap.sh"},
            ),
            expect_equal(
                "bare env with payload resolves to both preload entries",
                dx_harness_env(None, "my/pkg/test.sh"),
                {"DX_BOOTSTRAP": "_main/tools/sh/bootstrap.sh", "DX_HARNESS_SRC": "my/pkg/test.sh"},
            ),
            expect_equal(
                "caller env keeps its entries plus the payload entry",
                dx_harness_env({"FOO": "bar"}, "my/pkg/test.sh"),
                {"FOO": "bar", "DX_BOOTSTRAP": "_main/tools/sh/bootstrap.sh", "DX_HARNESS_SRC": "my/pkg/test.sh"},
            ),
            expect_equal(
                "explicit payload is not overwritten",
                dx_harness_env({"DX_HARNESS_SRC": "custom/probe.sh"}, "my/pkg/test.sh"),
                {"DX_BOOTSTRAP": "_main/tools/sh/bootstrap.sh", "DX_HARNESS_SRC": "custom/probe.sh"},
            ),
            expect_equal(
                "relative payload resolves against the caller package",
                dx_harness_entry("my/pkg", ["test.sh"]),
                {"srcs": ["//tools/sh:entry.sh"], "data": ["test.sh"], "rel": "my/pkg/test.sh"},
            ),
            expect_equal(
                "colon payload resolves against the caller package",
                dx_harness_entry("my/pkg", [":test.sh"]),
                {"srcs": ["//tools/sh:entry.sh"], "data": [":test.sh"], "rel": "my/pkg/test.sh"},
            ),
            expect_equal(
                "absolute payload resolves to its workspace relpath",
                dx_harness_entry("other/pkg", ["//my/pkg:test.sh"]),
                {"srcs": ["//tools/sh:entry.sh"], "data": ["//my/pkg:test.sh"], "rel": "my/pkg/test.sh"},
            ),
        ],
    )
