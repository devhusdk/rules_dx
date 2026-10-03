"""Unit tests for the visibility contract (single source)."""

load("//libs/starlark:defs.bzl", "expect_equal", "expect_false", "expect_true", "starlark_test")
load(":visibility.bzl", "DX_FACADE_PACKAGE", "EXPLICIT_PUBLIC_EXPORTS", "EXPLICIT_PUBLIC_TARGETS", "LAYER_FORBIDDEN_DEPS", "PRIVATE_ENV_PACKAGES", "PRIVATE_GAZELLE_PACKAGES", "PUBLIC_PACKAGES", "SCOPED_TARGET_GRANTS", "VIS_CLI", "VIS_INTERNAL", "VIS_PRIVATE", "VIS_PUBLIC", "VIS_QUALITY", "is_private_package", "is_public_package", "scoped_constant")

def _sorted(values):
    return sorted(values)

def visibility_unit_tests(name):
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal(
                "public scope stays one entry",
                VIS_PUBLIC,
                ["//visibility:public"],
            ),
            expect_equal(
                "private scope stays one entry",
                VIS_PRIVATE,
                ["//visibility:private"],
            ),
            expect_equal(
                "internal scope stays one entry",
                VIS_INTERNAL,
                ["//:__subpackages__"],
            ),
            expect_equal(
                "cli scope stays cli-only",
                VIS_CLI,
                ["//cli:__pkg__", "//cli:__subpackages__"],
            ),
            expect_equal(
                "quality scope stays quality-only",
                VIS_QUALITY,
                ["//quality:__pkg__", "//quality:__subpackages__"],
            ),
            expect_equal(
                "public packages stay the 24-package external API",
                len(PUBLIC_PACKAGES),
                24,
            ),
            expect_equal(
                "public packages stay sorted for buildifier parity",
                _sorted(PUBLIC_PACKAGES),
                PUBLIC_PACKAGES,
            ),
            expect_equal(
                "public packages admit the language rule wrappers",
                [is_public_package("rust/rules"), is_public_package("powershell/rules"), is_public_package("ruby/rules")],
                [True, True, True],
            ),
            expect_equal(
                "public packages keep the facade out (dx defaults private)",
                is_public_package(DX_FACADE_PACKAGE),
                False,
            ),
            expect_equal(
                "private env packages stay the 17 focused plans",
                len(PRIVATE_ENV_PACKAGES),
                17,
            ),
            expect_equal(
                "private env packages stay sorted for buildifier parity",
                _sorted(PRIVATE_ENV_PACKAGES),
                PRIVATE_ENV_PACKAGES,
            ),
            expect_equal(
                "private gazelle packages stay the 17 generated leaves",
                len(PRIVATE_GAZELLE_PACKAGES),
                17,
            ),
            expect_equal(
                "scoped packages resolve through the contract",
                [scoped_constant("cli/path"), scoped_constant("generation/codegen_shard"), scoped_constant("quality/result")],
                ["VIS_CLI_WIDE", "VIS_CLI_GENERATION", "VIS_QUALITY_CLI"],
            ),
            expect_equal(
                "cli binary package stays cli-scoped",
                scoped_constant("cli/cli"),
                "VIS_CLI",
            ),
            expect_equal(
                "unknown packages resolve empty",
                scoped_constant("examples/adopt-rust"),
                "",
            ),
            expect_true(
                "private packages admit the focused env plans",
                is_private_package("rust/env"),
            ),
            expect_true(
                "private packages admit the facade default",
                is_private_package(DX_FACADE_PACKAGE),
            ),
            expect_false(
                "private packages reject the public roots",
                is_private_package("env"),
            ),
            expect_equal(
                "explicit public targets stay binaries plus facade plus deploy tool",
                sorted(EXPLICIT_PUBLIC_TARGETS.keys()),
                ["cli/cli", "cli/env", "deploy/rules", "dx"],
            ),
            expect_equal(
                "explicit public exports stay the two Cargo workspaces",
                EXPLICIT_PUBLIC_EXPORTS,
                ["cli/cli", "cli/env"],
            ),
            expect_equal(
                "layering forbids the dx/cli/tools crossings",
                len(LAYER_FORBIDDEN_DEPS),
                9,
            ),
            expect_equal(
                "target grants stay the two CI-owned fixture exports",
                SCOPED_TARGET_GRANTS,
                [
                    ["quality/artifacts", "exports_files", "//tools/ci:__pkg__"],
                    ["quality/testdata", "exports_files", "//tools/ci:__pkg__"],
                ],
            ),
            expect_equal(
                "layering keeps every rule a triple",
                [len(rule) == 3 for rule in LAYER_FORBIDDEN_DEPS],
                [True, True, True, True, True, True, True, True, True],
            ),
            expect_equal(
                "scope tables stay disjoint",
                [
                    len([pkg for pkg in PUBLIC_PACKAGES if is_private_package(pkg)]),
                    len([pkg for pkg in PUBLIC_PACKAGES if scoped_constant(pkg) != ""]),
                    len([pkg for pkg in PRIVATE_ENV_PACKAGES if is_public_package(pkg)]),
                ],
                [0, 0, 0],
            ),
        ],
    )
