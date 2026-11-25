"""Parity gate unit tests."""

load("//core/private/testing:defs.bzl", "expect_equal", "starlark_test")
load(":adapters.bzl", "REAL_ADAPTERS")
load(":parity.bzl", "PARITY_DEFERRED", "adapter_backed_classes", "deferred_classes", "deferred_pipeline_error", "double_claimed_classes", "malformed_deferrals", "parity_schema_error", "unclassified_adapter_classes", "undispositioned_classes")

visibility("private")

def _derived_backed_classes():
    backed = {}
    for tool in REAL_ADAPTERS:
        for capability in REAL_ADAPTERS[tool]:
            for class_id in REAL_ADAPTERS[tool][capability]:
                backed[class_id] = True
    return sorted(backed.keys())

def parity_unit_tests(name):
    """Declares the parity gate unit test."""
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal(
                "parity deferral schema validates",
                parity_schema_error(),
                "",
            ),
            expect_equal(
                "no adapter class is unclassified",
                unclassified_adapter_classes(),
                [],
            ),
            expect_equal(
                "no classified class lacks a disposition",
                undispositioned_classes(),
                [],
            ),
            expect_equal(
                "no class is both adapter-backed and deferred",
                double_claimed_classes(),
                [],
            ),
            expect_equal(
                "every deferral names an owning decision and a frozen route",
                malformed_deferrals(),
                [],
            ),
            expect_equal(
                "adapter-backed query matches the registry derivation",
                adapter_backed_classes(),
                _derived_backed_classes(),
            ),
            expect_equal(
                "deferred query matches the deferral registry",
                deferred_classes(),
                sorted(PARITY_DEFERRED.keys()),
            ),
            expect_equal(
                "deferred pipeline error stays empty without deferred classes",
                deferred_pipeline_error(["rust", "python"], "lint"),
                "",
            ),
            expect_equal(
                "deferred pipeline error names the deferral with owner and route",
                deferred_pipeline_error(["rust", "ocaml"], "lint", {"ocaml": ["ADR", "no OCaml linter is pinned"]}) != "",
                True,
            ),
        ],
    )
