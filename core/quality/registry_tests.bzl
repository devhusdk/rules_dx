"""Unit tests for the single-sourced registry."""

load("//core:defs.bzl", "expect_equal", "starlark_test")
load(":adapters.bzl", "not_applicable_error")
load(":taxonomy.bzl", "CLASS_TO_FAMILY", "FAMILY_OWNERS", "TAXONOMY", "TAXONOMY_CLASSES", "taxonomy_schema_error")
load(":registry.bzl", "is_curated_family", "is_registry_class", "is_registry_tool", "registry_classes", "registry_curated_families", "registry_families", "registry_schema_error", "registry_tools")

_A = {"ty": {"typecheck": ["python"]}}
_C = {"python": "python", "yaml": "yaml"}

def registry_unit_tests(name):
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal(
                "taxonomy rejects a class under two families",
                taxonomy_schema_error({"a": struct(owner = "o", classes = ["x"]), "b": struct(owner = "o", classes = ["x"])}),
                "taxonomy: class 'x' listed under both 'a' and 'b'",
            ),
            expect_equal(
                "taxonomy rejects a family without classes",
                taxonomy_schema_error({"a": struct(owner = "o", classes = [])}),
                "taxonomy: family 'a' has no classes",
            ),
            expect_equal(
                "taxonomy rejects an empty owner",
                taxonomy_schema_error({"a": struct(owner = "", classes = ["x"])}),
                "taxonomy: family 'a' has an empty owner",
            ),
            expect_equal(
                "taxonomy tables share key sets",
                [sorted(CLASS_TO_FAMILY.keys()), sorted(FAMILY_OWNERS.keys())],
                [TAXONOMY_CLASSES, sorted(TAXONOMY.keys())],
            ),
            expect_equal(
                "aggregated registry schemas validate",
                registry_schema_error(),
                "",
            ),
            expect_equal(
                "registry queries stay non-empty",
                [
                    len(registry_classes()) > 0,
                    len(registry_families()) > 0,
                    len(registry_tools()) > 0,
                    len(registry_curated_families()) > 0,
                ],
                [True, True, True, True],
            ),
            expect_equal(
                "registry queries admit the frozen core",
                [
                    is_registry_class("rust"),
                    is_registry_class("python"),
                    is_registry_tool("ruff"),
                    is_registry_tool("biome"),
                    is_curated_family("python"),
                    is_curated_family("rust"),
                ],
                [True, True, True, True, True, True],
            ),
            expect_equal(
                "registry queries reject unknown IDs",
                [
                    is_registry_class("not_a_class"),
                    is_registry_tool("not_a_tool"),
                    is_curated_family("not_a_family"),
                ],
                [False, False, False],
            ),
            expect_equal(
                "not-applicable validation accepts and rejects",
                [
                    not_applicable_error({"typecheck": {"yaml": "data"}}, _A, _C),
                    not_applicable_error({"bogus": {"yaml": "data"}}, _A, _C),
                    not_applicable_error({"typecheck": {"nope": "data"}}, _A, _C),
                    not_applicable_error({"typecheck": {"yaml": ""}}, _A, _C),
                    not_applicable_error({"typecheck": {"python": "x"}}, _A, _C),
                    not_applicable_error({"format": {"yaml": "data"}}, _A, _C),
                    not_applicable_error({"format": {"python": "x"}}, {"ruff": {"format": ["python"]}}, _C),
                ],
                [
                    "",
                    "not applicable: unknown capability 'bogus'",
                    "not applicable: unknown family 'nope' for typecheck",
                    "not applicable: empty reason for typecheck:yaml",
                    "not applicable: typecheck:python contradicts tool 'ty'",
                    "",
                    "not applicable: format:python contradicts tool 'ruff'",
                ],
            ),
            expect_equal(
                "javac is a registry tool",
                is_registry_tool("javac"),
                True,
            ),
            expect_equal(
                "flang, ldc, ocaml, swiftc and zig are registry tools",
                [is_registry_tool("flang"), is_registry_tool("ldc"), is_registry_tool("ocaml"), is_registry_tool("swiftc"), is_registry_tool("zig")],
                [True, True, True, True, True],
            ),
            expect_equal(
                "scalac is a registry tool",
                is_registry_tool("scalac"),
                True,
            ),
            expect_equal(
                "kotlinc is a registry tool",
                is_registry_tool("kotlinc"),
                True,
            ),
            expect_equal(
                "tsc is a registry tool",
                is_registry_tool("tsc"),
                True,
            ),
        ],
    )
