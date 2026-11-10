"""Gate: one owner per concern and class, one formatter per class."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load(":adapters.bzl", "REAL_CLASS_TO_FAMILY")
load(":curated_defaults.bzl", "CURATED_DEFAULTS")
load(":overlaps.bzl", "owners")
load(":tool_defs.bzl", "tool")
load(":tools.bzl", "TOOLS")

# Alternatives in the catalog: the same concern and class, never selected together in a family.
EXPECTED_ALTERNATIVES = {
    "format/helm": ["helmfmt", "prettier"],
    "lint/javascript": ["biome", "eslint"],
    "lint/jsx": ["biome", "eslint"],
    "lint/tsx": ["biome", "eslint"],
    "lint/typescript": ["biome", "eslint"],
}

def _message(concern, class_id, ids):
    return "class %s, concern %s: owned by %s" % (class_id, concern, " and ".join(ids))

def catalog_problems(tools, expected):
    """Returns the catalog overlaps that are not in the expected-alternatives table."""
    found = owners(tools)
    problems = [_message(key.split("/")[0], key.split("/")[1], ids) for key, ids in sorted(found.items()) if expected.get(key) != ids]
    problems += ["expected alternatives %s are no longer an overlap" % key for key in sorted(expected) if key not in found]
    return problems

def family_problems(tools, defaults):
    """Returns the curated families that give one class two owners for a concern."""
    by_id = {t.id: t for t in tools}
    problems = []
    for family, entry in sorted(defaults.items()):
        claims = {}
        for capability, selected in entry.items():
            for tool_id in selected:
                tool_row = by_id[tool_id]
                for class_id in tool_row.adapters.get(capability, []):
                    if REAL_CLASS_TO_FAMILY.get(class_id) == family:
                        claims.setdefault((tool_row.concerns[capability], class_id), []).append(tool_id)
        for (concern, class_id), ids in sorted(claims.items()):
            if len(set(ids)) > 1:
                problems.append("family %s: %s" % (family, _message(concern, class_id, sorted(set(ids)))))
    return problems

def formatter_problems(tools, defaults):
    """Returns the selected lint tools that format a class another tool formats in that family."""
    by_id = {t.id: t for t in tools}
    problems = []
    for family, entry in sorted(defaults.items()):
        for tool_id in entry.get("lint", []):
            for class_id in by_id[tool_id].adapters.get("format", []):
                if REAL_CLASS_TO_FAMILY.get(class_id) == family and tool_id not in entry.get("format", []):
                    problems.append("family %s: class %s, concern format: %s lints and formats but is not the family formatter %s" % (family, class_id, tool_id, entry.get("format", [])))
    return problems

def _fake(id, **adapters):
    return tool(id = id, adapters = adapters)

def overlap_unit_tests(name):
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal("catalog overlaps are the expected alternatives", catalog_problems(TOOLS, EXPECTED_ALTERNATIVES), []),
            expect_equal("curated families give each concern one owner per class", family_problems(TOOLS, CURATED_DEFAULTS), []),
            expect_equal("a linting formatter is its family's formatter", formatter_problems(TOOLS, CURATED_DEFAULTS), []),
            expect_equal(
                "a second formatter for a class names both tools",
                catalog_problems([_fake("fmt_a", format = ["json"]), _fake("fmt_b", format = ["json"])], {}),
                ["class json, concern format: owned by fmt_a and fmt_b"],
            ),
            expect_equal(
                "a distinct concern does not overlap",
                catalog_problems([_fake("lint_a", lint = ["go"]), tool(id = "vet_a", adapters = {"lint": ["go"]}, concerns = {"lint": "vet"})], {}),
                [],
            ),
            expect_equal(
                "selecting two tools for one concern fails the family",
                family_problems(
                    [_fake("lint_a", lint = ["go"]), _fake("lint_b", lint = ["go"])],
                    {"go": {"lint": ["lint_a", "lint_b"], "format": []}},
                ),
                ["family go: class go, concern lint: owned by lint_a and lint_b"],
            ),
        ],
    )
