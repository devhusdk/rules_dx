"""Schema tests for the support matrix facts."""

load("//libs/starlark:defs.bzl", "expect_equal", "expect_true", "starlark_test")
load(":facts.bzl", "support_matrix_facts_dict")
load("//quality:adapters.bzl", "NOT_APPLICABLE")
load("//quality:support.bzl", "SUPPORT_PLATFORMS")
load(":languages.bzl", "PLATFORMS")

def _families_resolve(facts):
    return [
        row["name"] + ":" + family
        for row in facts["rows"]
        for family in row["families"]
        if family not in facts["quality"]
    ]

def _not_applicable_problems(facts):
    return [
        capability + ":" + family
        for capability, entries in facts["not_applicable"].items()
        for family in entries
        if family not in facts["quality"] or facts["quality"][family].get(capability, []) != []
    ]

def support_matrix_facts_tests(name):
    """Declares the support matrix facts schema test."""
    facts = support_matrix_facts_dict()
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal("facts schema stays v1", facts["schema_version"], 1),
            expect_equal(
                "platform order is fixed",
                [platform["id"] for platform in facts["platforms"]],
                [platform["id"] for platform in PLATFORMS],
            ),
            expect_equal(
                "support platform vocabulary matches the matrix platforms",
                SUPPORT_PLATFORMS,
                [platform["id"] for platform in PLATFORMS],
            ),
            expect_equal("every family resolves", _families_resolve(facts), []),
            expect_equal(
                "rows are sorted by name",
                [row["name"] for row in facts["rows"]],
                sorted([row["name"] for row in facts["rows"]]),
            ),
            expect_equal("not applicable families have no tools", _not_applicable_problems(facts), []),
            expect_equal(
                "typecheck n/a families are an explicit snapshot",
                sorted(NOT_APPLICABLE["typecheck"].keys()),
                ["ansible", "api_spec", "dockerfile", "gherkin", "html_template", "jsonnet", "kubernetes", "latex", "make", "nix", "perl", "powershell", "sql", "typst", "xml", "yaml"],
            ),
            expect_true("rows are listed", len(facts["rows"]) > 0),
        ],
    )
